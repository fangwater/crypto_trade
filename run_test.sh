#!/bin/bash

echo "构建项目..."
cargo build --release

echo "启动Signal Collector..."
cargo run --release --bin signal-collector &
COLLECTOR_PID=$!

sleep 2

echo "启动IceOryx信号生成器..."
cargo run --release --bin ice-signal-generator &
ICE_PID=$!

echo "启动ZMQ信号生成器..."
cargo run --release --bin zmq-signal-generator &
ZMQ_PID=$!

echo "所有进程已启动"
echo "按 Ctrl+C 停止测试"

# 捕获中断信号
trap "echo '停止所有进程...'; kill $COLLECTOR_PID $ICE_PID $ZMQ_PID 2>/dev/null; exit" INT

# 等待
wait