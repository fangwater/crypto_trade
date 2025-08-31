#!/bin/bash

echo "启动IceOryx2测试环境..."

# 编译项目
echo "编译项目..."
cargo build

# 启动信号收集器
echo "启动信号收集器..."
cargo run --bin signal-collector &
COLLECTOR_PID=$!

# 等待收集器启动
sleep 2

# 启动IceOryx信号生成器
echo "启动IceOryx信号生成器..."
cd test-signal-generator
cargo run --bin ice-signal-generator &
GENERATOR_PID=$!

echo "IceOryx2测试环境已启动"
echo "信号收集器 PID: $COLLECTOR_PID"
echo "信号生成器 PID: $GENERATOR_PID"
echo ""
echo "按 Ctrl+C 停止测试..."

# 等待中断信号
trap 'echo "停止所有进程..."; kill $COLLECTOR_PID $GENERATOR_PID 2>/dev/null; exit' INT

# 保持脚本运行
while true; do
    sleep 1
done
