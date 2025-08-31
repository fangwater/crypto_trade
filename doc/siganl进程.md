graph LR
    subgraph "INPUT [输入接口]"
        I1[IPC Subscribe<br/>iceoryx2订阅]
        I2[ZMQ Subscribe<br/>备用通道]
        
        subgraph "Signal Types"
            ST1[MarketData<br/>100ms]
            ST2[FundingRate<br/>60s]
            ST3[PositionClose<br/>1s]
            ST4[OrderResponse<br/>Event]
        end
        
        I1 --> ST1
        I1 --> ST2
        I1 --> ST3
        I2 --> ST4
    end
    
    subgraph "Signal Collector Process"
        subgraph "Core Components"
            SM[Signal Manager<br/>信号状态管理]
            TR[Trigger Registry<br/>触发器注册表]
            EV[Event Generator<br/>事件生成器]
            CH[Channel Manager<br/>通道管理器]
        end
        
        subgraph "State Storage"
            SS[Signal State<br/>HashMap<SignalKey, Signal>]
            TS[Trigger State<br/>Vec<TriggerContext>]
            ES[Event Queue<br/>PriorityQueue<Event>]
        end
        
        subgraph "Processing Logic"
            PL1[Signal Update]
            PL2[Trigger Evaluation]
            PL3[Event Generation]
            PL4[Priority Assignment]
        end
        
        SM --> SS
        TR --> TS
        EV --> ES
        
        PL1 --> PL2 --> PL3 --> PL4
    end
    
    subgraph "OUTPUT [输出接口]"
        O1[IPC Publish<br/>to Pre-Process]
        
        subgraph "Event Types"
            ET1[TradingEvent<br/>开仓/平仓]
            ET2[HedgeEvent<br/>对冲指令]
            ET3[ControlEvent<br/>风控事件]
            ET4[AlertEvent<br/>告警事件]
        end
        
        ET1 --> O1
        ET2 --> O1
        ET3 --> O1
        ET4 --> O1
    end
    
    ST1 --> SM
    ST2 --> SM
    ST3 --> SM
    ST4 --> SM
    
    CH --> O1
    
    style I1 fill:#2196F3,color:#fff
    style SM fill:#667eea,color:#fff
    style O1 fill:#4CAF50,color:#fff