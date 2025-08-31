#!/bin/bash

echo "启动高频交易系统测试"
echo "===================="

# 启动Signal Collector
RUST_LOG=info cargo run --bin signal-collector 2>&1 | grep -E "(Starting|Published|signal|event)" &
PID1=$!

sleep 2

# 启动测试信号生成器
RUST_LOG=info cargo run --bin ice-signal-generator 2>&1 | grep -E "(发送|Starting)" &
PID2=$!

RUST_LOG=info cargo run --bin zmq-signal-generator 2>&1 | grep -E "(发送|Starting)" &
PID3=$!

echo "系统已启动，监控10秒..."
sleep 10

echo "停止所有进程..."
kill $PID1 $PID2 $PID3 2>/dev/null
pkill -f signal-collector
pkill -f ice-signal-generator  
pkill -f zmq-signal-generator

echo "测试完成"