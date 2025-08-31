# 高频交易系统crypto_trade

## 项目初始化提示词

```
我需要创建一个基于Rust的高频交易系统，采用N+1+1+1的多进程架构。系统包含3个核心进程：
1. Signal Collector - 信号收集和触发器评估
2. Pre/Post Processor - 风控检查和仓位管理
3. Trading Engine - 订单执行和WebSocket管理

技术栈：
- Rust异步：tokio (单线程运行时)
- IPC通信：iceoryx2 (零拷贝)
- 序列化：bincode/serde
- WebSocket：tokio-tungstenite

请帮我创建项目结构，包括：
- workspace配置，3个独立的crate
- 共享的types和messages库
- 每个进程的基础框架
- IPC通信的基础封装
```

## 1. Signal Collector 进程开发

### 1.1 基础框架
```
创建Signal Collector进程的基础框架：

功能需求：
- 单线程tokio异步运行时
- 接收来自IPC的信号
- [1] AdaptiveSpreadDeviationSignal 自适应价差偏离信号，根据币对盘口价差百分位
- [2] FixedSpreadDeviationSignal 固定价差偏离信号，根据币对盘口价差的固定阈值
- [3] FundingRateDirectionSignal 资金费率方向信号 开仓决策
- [4] RealTimeFundingRiskSignal 资金费率风险信号 风控平仓
- [5] OrderResponse //待定

- 维护信号状态Vec[signal_idx] = SignalStatus
- 维护一组trigger Vec[trigger_idx] = rc->trigger

SignalStatus 每个signal维护一个list of trigger（idx vec）。即所有会因为这个信号变更，可能触发的tigger
当信号输入的时候，代表信号更新，依次检查所有idx对应trigger 

- tigger可能触发生TradingEvent并发送到Pre/Post Process

关键数据结构：
1. Signal状态管理器
2. Trigger trait和注册表
3. Event生成器
4. MPSC channel通信 多个signal输入，监听iceoryx2、zmq ipc的消息。推送到一个channel。
这个rust进程，是一个基于消息事件触发的dispatcher。是一个单线程的，纯tokio。

请实现：
1. main.rs with tokio runtime setup
2. signal_manager.rs with state management
3. trigger.rs with Trigger trait and implementations
4. event_generator.rs for event creation
```

### 1.2 Trigger系统实现
```
实现Signal Collector的Trigger系统：

需要实现的Trigger类型：
1. MTTrigger - MT策略触发器
2. MTCloseTrigger - 平仓触发器
3. HedgeTrigger - 对冲触发器


实现要求：
- Trigger trait with evaluate() method
- 支持依赖信号查询state
- 优先级设置

请提供完整的trigger模块实现。
```

### 1.3 IPC集成
```
为Signal Collector集成IPC通信：

输入端：
- 订阅iceoryx2的多个topic
- 反序列化都是纯二进制

输出端：
- 发布TradingEvent到Pre/Post Process
- 支持优先级队列

实现要求：
1. ipc_subscriber.rs - 订阅者实现
2. ipc_publisher.rs - 发布者实现
3. 消息路由和分发
4. 错误处理和重连机制
5. 背压处理

注意零拷贝优化和延迟控制。
```
Signal Collector 现在帮我实现这个，并编写一个测试。
测试的目的是，模拟输入信号，构建一个模拟输入3个信号进程，然后用ice和zmq的ipc输入信号到signal collector，通过trigger判断和触发，最后变成event


## 2. Pre/Post Processor 进程开发

### 2.1 Pipeline框架
```
创建Pre/Post Processor的Pipeline框架：

架构设计：
- 单线程tokio，两个独立channel (pre/post)
- Pipeline模式，支持动态组合处理阶段
- 无锁共享状态管理

Pre-Process Pipeline阶段：
1. SignalAgeCheck - 信号时效性验证
2. RiskControl - 风控检查
3. PositionLimit - 仓位限制
4. OrderConstruction - 订单构造
5. PriorityAssignment - 优先级分配

Post-Process Pipeline阶段：
1. PositionUpdate - 仓位更新
2. RiskQuotaUpdate - 风控额度更新
3. HedgeTriggerCheck - 对冲检查
4. PnLCalculation - 盈亏计算
5. StatePersistence - 状态持久化

请实现：
1. pipeline.rs - Pipeline trait和框架
2. pre_stages.rs - Pre-process各阶段
3. post_stages.rs - Post-process各阶段
4. shared_state.rs - 共享状态管理
```

### 2.2 风控系统
```
实现完整的风控检查系统：

风控规则：
1. 单品种限制
   - max_position: 100手
   - max_capital: 5000 USDT
   - max_pending_orders: 3

2. 组合限制
   - max_total_exposure: 0.03
   - warning_threshold: 0.025
   - max_daily_trades: 1000

3. 时间限制
   - cooldown_seconds: 60
   - signal_max_age_ms: 100

实现要求：
- RiskRule trait
- 规则链式执行
- 快速失败机制
- 风控状态原子更新
- 实时指标计算

请实现risk_control模块，包括：
1. risk_rules.rs - 各种规则实现
2. risk_state.rs - 风控状态管理
3. risk_calculator.rs - 风险指标计算
```

### 2.3 订单管理
```
实现订单管理系统：

订单状态机：
Created -> Validated -> Submitting -> Submitted -> Acknowledged 
-> PartiallyFilled -> Filled / Cancelled / Rejected

功能需求：
1. 订单生命周期管理
2. ClientOrderId生成（幂等）
3. 订单簿维护
4. 套利组合管理
5. 对冲关联

数据结构：
- Order with state machine
- OrderBook with indexes
- ArbitragePair for MT positions
- Fill tracking

请实现：
1. order.rs - 订单数据结构
2. order_manager.rs - 订单管理器
3. order_state.rs - 状态机实现
4. arbitrage.rs - 套利组合管理
```

## 3. Trading Engine 进程开发

### 3.1 WebSocket连接池
```
实现Trading Engine的WebSocket连接池：

需求：
- 每个交易所维护多个WebSocket连接
- Spot: 3个连接, Futures: 3个连接
- 健康度评分机制
- 自动重连和故障转移

WebSocket管理：
1. 连接健康度评分（0-100）
   - RTT延迟
   - 成功率
   - 最近错误

2. 智能路由选择
   - 根据健康度选择最优连接
   - 支持多路并发发送
   - 故障自动切换

3. 连接维护
   - 心跳检测
   - 指数退避重连
   - 连接池动态调整

请实现：
1. ws_pool.rs - 连接池管理
2. ws_connection.rs - 单个连接封装
3. health_tracker.rs - 健康度监控
4. connection_selector.rs - 连接选择算法
```

### 3.2 订单执行器
```
实现核心订单执行器：

执行流程：
1. 接收ExecutionCommand
2. 选择健康的WebSocket连接（2/3）
3. 构造订单请求
4. 生成签名
5. 设置ClientOrderId（幂等）
6. 并发发送到多个连接
7. 处理响应

关键特性：
- 幂等保证（利用ClientOrderId）
- 多路并发发送
- 至少一个成功即可
- 无状态设计

实现模块：
1. executor.rs - 执行器主逻辑
2. order_builder.rs - 订单构造
3. signer.rs - 签名生成
4. idempotent.rs - 幂等机制
5. response_handler.rs - 响应处理
```

### 3.3 交易所适配器
```
实现交易所适配层：

支持的交易所：
1. Binance (Spot + Futures)
2. OKEx (可选)
3. Bybit (可选)

适配器功能：
- 统一的订单格式转换
- 交易所特定的签名算法
- 响应解析和标准化
- 错误码映射

请实现：
1. exchange_adapter.rs - 适配器trait
2. binance_adapter.rs - 币安实现
3. message_parser.rs - 消息解析
4. error_mapper.rs - 错误映射
```

## 4. 系统集成与测试

### 4.1 集成测试
```
创建完整的集成测试：

测试场景：
1. MT策略完整流程测试
   - 信号触发 -> 开仓 -> 对冲 -> 平仓
   
2. 异常处理测试
   - WebSocket断线重连
   - 订单超时重试
   - 对冲失败处理

3. 性能测试
   - 延迟测试（目标<100μs）
   - 吞吐量测试（>10K orders/s）
   - 内存占用测试

请创建：
1. tests/integration_test.rs
2. tests/performance_test.rs
3. mock交易所实现
4. 测试数据生成器
```

### 4.2 监控和日志
```
添加监控和日志系统：

监控指标：
- 各阶段处理延迟
- 订单成功率
- WebSocket健康度
- 内存和CPU使用

日志要求：
- 使用tracing框架
- 结构化日志
- 异步日志输出
- 日志分级和采样

实现：
1. metrics.rs - 指标收集
2. logging.rs - 日志配置
3. monitoring.rs - 监控接口
```

## 5. 配置和部署

### 5.1 配置管理
```
创建配置管理系统：

配置文件格式：YAML
配置热更新支持
环境变量覆盖

配置结构：
- exchanges: 交易所配置
- risk_control: 风控参数
- strategy: 策略参数
- system: 系统配置

请实现：
1. config.rs - 配置结构定义
2. config_loader.rs - 配置加载
3. 示例配置文件
```

### 5.2 启动脚本
```
创建系统启动和管理脚本：

1. 进程启动顺序控制
2. 健康检查
3. 优雅关闭
4. 日志收集
5. 性能监控

提供systemd service文件和docker-compose配置。
```

## 使用说明

1. **按顺序使用提示词**：从项目初始化开始，逐步构建每个组件
2. **模块化开发**：每个提示词聚焦一个具体模块
3. **增量测试**：每完成一个模块就进行单元测试
4. **性能优先**：始终关注延迟和内存使用
5. **错误处理**：每个模块都要有完善的错误处理

## 注意事项

- 所有进程使用单线程tokio运行时
- 优先使用无锁数据结构
- IPC通信使用零拷贝
- 关键路径避免内存分配
- 使用对象池减少GC压力
- 日志输出异步化，避免阻塞主流程