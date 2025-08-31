#!/bin/bash

echo "===================="
echo "高频交易系统完整测试"
echo "===================="
echo ""

# 清理
echo "1. 清理环境..."
pkill -f signal-collector 2>/dev/null
pkill -f ice-signal-generator 2>/dev/null
pkill -f zmq-signal-generator 2>/dev/null
./clean_ipc.sh

sleep 1

echo ""
echo "2. 启动Signal Collector..."
RUST_LOG=info cargo run --release --bin signal-collector 2>&1 | grep -E "(Starting|Published|received|event)" &
COLLECTOR_PID=$!

sleep 2

echo ""
echo "3. 启动IceOryx信号生成器..."
RUST_LOG=info cargo run --release --bin ice-signal-generator 2>&1 | grep -E "(Starting|发送)" &
ICE_PID=$!

echo ""
echo "4. 启动ZMQ信号生成器..."
RUST_LOG=info cargo run --release --bin zmq-signal-generator 2>&1 | grep -E "(Starting|发送)" &
ZMQ_PID=$!

echo ""
echo "系统正在运行..."
echo "- IceOryx: 发送自适应价差和资金费率信号(每2秒)"
echo "- ZMQ: 发送固定价差和风险信号(每3秒)"
echo ""

sleep 15

echo ""
echo "5. 停止所有进程..."
kill $COLLECTOR_PID $ICE_PID $ZMQ_PID 2>/dev/null
pkill -f signal-collector 2>/dev/null
pkill -f ice-signal-generator 2>/dev/null
pkill -f zmq-signal-generator 2>/dev/null

echo ""
echo "测试完成！"