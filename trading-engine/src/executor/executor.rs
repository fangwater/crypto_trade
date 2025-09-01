use super::{
    idempotent::IdempotentManager,
    order_builder::OrderBuilder,
    response_handler::ResponseHandler,
    signer::Signer,
    types::*,
};
use crate::config::ExecutorConfig;
use crate::health::{ConnectionSelector, HealthTracker};
use crate::ws_pool::WsPool;
use futures::future::join_all;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

pub struct OrderExecutor {
    config: ExecutorConfig,
    ws_pool: Arc<WsPool>,
    health_tracker: Arc<HealthTracker>,
    connection_selector: Arc<ConnectionSelector>,
    idempotent_manager: Arc<IdempotentManager>,
    signers: Arc<dashmap::DashMap<String, Signer>>,
}

impl OrderExecutor {
    pub fn new(
        config: ExecutorConfig,
        ws_pool: Arc<WsPool>,
        health_tracker: Arc<HealthTracker>,
        connection_selector: Arc<ConnectionSelector>,
    ) -> Self {
        let idempotent_manager = Arc::new(IdempotentManager::new(config.idempotent_key_prefix.clone()));
        
        Self {
            config,
            ws_pool,
            health_tracker,
            connection_selector,
            idempotent_manager,
            signers: Arc::new(dashmap::DashMap::new()),
        }
    }

    pub fn register_signer(&self, exchange: String, api_key: String, secret_key: String) {
        let signer = Signer::new(api_key, secret_key);
        self.signers.insert(exchange, signer);
    }

    pub async fn execute(&self, command: ExecutionCommand) -> ExecutionResult {
        info!("Executing order command: {:?}", command.id);
        
        // Generate idempotent client order ID
        let client_order_id = if let Some(ref id) = command.client_order_id {
            id.clone()
        } else {
            self.idempotent_manager.generate_client_order_id(command.id)
        };

        // Check for duplicate
        if self.idempotent_manager.is_duplicate(&client_order_id) {
            warn!("Duplicate order detected: {}", client_order_id);
            return ExecutionResult {
                command_id: command.id,
                success: false,
                responses: vec![],
                selected_response: None,
                error: Some("Duplicate order".to_string()),
            };
        }

        // Get signer for exchange
        let signer = match self.signers.get(&command.exchange) {
            Some(s) => s.value().clone(),
            None => {
                error!("No signer configured for exchange: {}", command.exchange);
                return ExecutionResult {
                    command_id: command.id,
                    success: false,
                    responses: vec![],
                    selected_response: None,
                    error: Some(format!("No signer for {}", command.exchange)),
                };
            }
        };

        // Build order request
        let order_builder = OrderBuilder::new(command.exchange.clone());
        let order_request = order_builder.build_order_request(&command, client_order_id.clone(), &signer);

        // Select healthy connections
        let connection_ids = self.connection_selector.select_connections(
            &command.exchange,
            &command.market_type,
            self.config.concurrent_send_count,
        );

        if connection_ids.is_empty() {
            error!("No healthy connections available for {} {}", command.exchange, command.market_type);
            return ExecutionResult {
                command_id: command.id,
                success: false,
                responses: vec![],
                selected_response: None,
                error: Some("No healthy connections".to_string()),
            };
        }

        // Send order to multiple connections concurrently
        let responses = self.send_concurrent(
            connection_ids,
            order_request,
            command.exchange.clone(),
        ).await;

        // Handle responses
        let response_handler = ResponseHandler::new(command.exchange.clone());
        let selected_response = response_handler.select_best_response(responses.clone());

        let success = selected_response.as_ref()
            .map(|r| matches!(r.status, OrderStatus::New | OrderStatus::PartiallyFilled | OrderStatus::Filled))
            .unwrap_or(false);

        ExecutionResult {
            command_id: command.id,
            success,
            responses,
            selected_response,
            error: if !success { Some("Order execution failed".to_string()) } else { None },
        }
    }

    async fn send_concurrent(
        &self,
        connection_ids: Vec<Uuid>,
        order_request: OrderRequest,
        exchange: String,
    ) -> Vec<OrderResponse> {
        let request_json = match serde_json::to_vec(&order_request) {
            Ok(json) => json,
            Err(e) => {
                error!("Failed to serialize order request: {}", e);
                return vec![];
            }
        };

        let mut futures = vec![];
        
        let client_order_id = order_request.client_order_id.clone();
        let symbol = order_request.symbol.clone();
        
        for conn_id in connection_ids {
            let pool = self.ws_pool.clone();
            let health_tracker = self.health_tracker.clone();
            let request = request_json.clone();
            let exchange = exchange.clone();
            let timeout_ms = self.config.order_timeout_ms;
            let client_order_id = client_order_id.clone();
            let symbol = symbol.clone();
            
            let future = async move {
                let start = std::time::Instant::now();
                
                match timeout(
                    Duration::from_millis(timeout_ms),
                    pool.send_to_connection(conn_id, request),
                ).await {
                    Ok(Ok(_)) => {
                        let rtt_ms = start.elapsed().as_secs_f64() * 1000.0;
                        health_tracker.update_success(conn_id, rtt_ms);
                        
                        // TODO: Wait for and parse actual response
                        // For now, return a placeholder
                        Some(OrderResponse {
                            order_id: Uuid::new_v4().to_string(),
                            client_order_id,
                            symbol,
                            status: OrderStatus::New,
                            executed_qty: rust_decimal::Decimal::ZERO,
                            executed_price: None,
                            timestamp: chrono::Utc::now().timestamp_millis(),
                            error: None,
                        })
                    }
                    Ok(Err(e)) => {
                        error!("Failed to send order to connection {}: {}", conn_id, e);
                        health_tracker.update_failure(conn_id);
                        None
                    }
                    Err(_) => {
                        error!("Order timeout for connection {}", conn_id);
                        health_tracker.update_failure(conn_id);
                        None
                    }
                }
            };
            
            futures.push(future);
        }

        let results = join_all(futures).await;
        results.into_iter().flatten().collect()
    }

    pub async fn retry_execution(&self, command: ExecutionCommand) -> ExecutionResult {
        let mut last_result = None;
        
        for attempt in 0..self.config.max_retry_attempts {
            debug!("Execution attempt {} for command {}", attempt + 1, command.id);
            
            let result = self.execute(command.clone()).await;
            
            if result.success {
                return result;
            }
            
            last_result = Some(result);
            
            if attempt < self.config.max_retry_attempts - 1 {
                tokio::time::sleep(Duration::from_millis(100 * (attempt as u64 + 1))).await;
            }
        }
        
        last_result.unwrap_or_else(|| ExecutionResult {
            command_id: command.id,
            success: false,
            responses: vec![],
            selected_response: None,
            error: Some("Max retry attempts exceeded".to_string()),
        })
    }
}