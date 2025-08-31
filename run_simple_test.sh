#!/bin/bash

echo "高频交易系统测试"
echo "================"

# 清理
pkill -f signal-collector 2>/dev/null
pkill -f zmq-signal-generator 2>/dev/null

echo "启动Signal Collector..."
RUST_LOG=info cargo run --bin signal-collector &
PID1=$!

sleep 2

echo "启动ZMQ信号生成器..."
RUST_LOG=info cargo run --bin zmq-signal-generator &
PID2=$!

echo ""
echo "系统运行中，观察10秒..."
sleep 10

echo "停止进程..."
kill $PID1 $PID2 2>/dev/null

echo "测试完成"