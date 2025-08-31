graph TB
    subgraph "INPUT Channels"
        IC1[Pre-Process Channel<br/>from Signal Collector]
        IC2[Post-Process Channel<br/>from Trading Engine]
        
        subgraph "Input Events"
            IE1[TradingEvent]
            IE2[HedgeEvent]
            IE3[OrderResponse]
            IE4[FillReport]
        end
        
        IC1 --> IE1
        IC1 --> IE2
        IC2 --> IE3
        IC2 --> IE4
    end
    
    subgraph "Pre/Post Processor"
        subgraph "Pre-Process Pipeline"
            PP1[Signal Validation<br/>时效性检查]
            PP2[Risk Check<br/>风控检查]
            PP3[Position Check<br/>仓位限制]
            PP4[Order Construction<br/>订单构造]
            PP5[Priority Queue<br/>优先级队列]
            
            PP1 --> PP2 --> PP3 --> PP4 --> PP5
        end
        
        subgraph "Shared State [Lock-Free]"
            RS[Risk State<br/>风控状态]
            PS[Position State<br/>仓位信息]
            OS[Order State<br/>订单状态]
            AS[Arbitrage State<br/>套利组合]
        end
        
        subgraph "Post-Process Pipeline"
            PO1[Position Update<br/>仓位更新]
            PO2[Risk Update<br/>风控更新]
            PO3[Hedge Check<br/>对冲检查]
            PO4[PnL Calculate<br/>盈亏计算]
            PO5[State Persist<br/>状态持久化]
            
            PO1 --> PO2 --> PO3 --> PO4 --> PO5
        end
        
        PP2 -.-> RS
        PP3 -.-> PS
        PP4 -.-> OS
        
        PO1 -.-> PS
        PO2 -.-> RS
        PO3 -.-> AS
    end
    
    subgraph "OUTPUT Channels"
        OC1[Execution Channel<br/>to Trading Engine]
        OC2[Hedge Channel<br/>to Signal Collector]
        
        subgraph "Output Commands"
            OE1[ExecutionCommand]
            OE2[CancelCommand]
            OE3[HedgeSignal]
            OE4[StateUpdate]
        end
        
        OE1 --> OC1
        OE2 --> OC1
        OE3 --> OC2
        OE4 --> OC2
    end
    
    IE1 --> PP1
    IE2 --> PP1
    IE3 --> PO1
    IE4 --> PO1
    
    PP5 --> OE1
    PO3 --> OE3
    
    style IC1 fill:#FF9800,color:#fff
    style RS fill:#FFC107,color:#000
    style OC1 fill:#667eea,color:#fff