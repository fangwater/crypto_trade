#!/bin/bash

echo "高频交易系统演示"
echo "=================="
echo ""

# 清理旧进程
pkill -f signal-collector
pkill -f ice-signal-generator
pkill -f zmq-signal-generator

echo "1. 启动 Signal Collector (信号收集器)..."
RUST_LOG=info cargo run --bin signal-collector 2>&1 | sed 's/^/[Collector] /' &
COLLECTOR_PID=$!

sleep 2

echo "2. 启动 IceOryx 信号生成器..."
RUST_LOG=info cargo run --bin ice-signal-generator 2>&1 | sed 's/^/[IceOryx] /' &
ICE_PID=$!

echo "3. 启动 ZMQ 信号生成器..."
RUST_LOG=info cargo run --bin zmq-signal-generator 2>&1 | sed 's/^/[ZMQ] /' &
ZMQ_PID=$!

echo ""
echo "系统正在运行..."
echo "- Signal Collector 监听来自 IceOryx 和 ZMQ 的信号"
echo "- 当满足触发条件时，会生成交易事件"
echo "- 按 Ctrl+C 停止"
echo ""

trap "echo '停止所有进程...'; kill $COLLECTOR_PID $ICE_PID $ZMQ_PID 2>/dev/null; exit" INT

wait