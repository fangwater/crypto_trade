#!/bin/bash

echo "Testing Trading Engine..."

# Create test config
cat > config/trading_engine_test.toml << EOF
[ws_pool]
max_connections_per_exchange = 1
heartbeat_interval_ms = 30000
reconnect_delay_ms = 1000
max_reconnect_attempts = 3
health_check_interval_ms = 5000
message_buffer_size = 100

[executor]
order_timeout_ms = 5000
max_retry_attempts = 2
concurrent_send_count = 2
idempotent_key_prefix = "TEST"

[ipc]
service_name = "trading_engine_test"
input_topic = "test_commands"
output_topic = "test_market_data"
buffer_size = 100

[exchanges.binance]
enabled = false
api_key = "test_api"
secret_key = "test_secret"

[exchanges.binance.spot]
enabled = false
ws_endpoints = ["wss://stream.binance.com:9443/ws"]
rest_endpoint = "https://api.binance.com"
connection_count = 1

[exchanges.binance.futures]
enabled = false
ws_endpoints = ["wss://fstream.binance.com/ws"]
rest_endpoint = "https://fapi.binance.com"
connection_count = 1
EOF

echo "Building Trading Engine..."
cargo build --bin trading-engine

echo "Starting Trading Engine (5 seconds test)..."
timeout 5 CONFIG_PATH=config/trading_engine_test.toml cargo run --bin trading-engine 2>&1 | head -20

echo "Test completed!"