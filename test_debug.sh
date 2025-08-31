#!/bin/bash

echo "清理环境..."
./clean_ipc.sh

echo "启动信号收集器..."
RUST_LOG=debug cargo run --bin signal-collector > collector.log 2>&1 &
COLLECTOR_PID=$!

sleep 2

echo "启动信号生成器..."
RUST_LOG=debug cargo run --bin ice-signal-generator > generator.log 2>&1 &
GENERATOR_PID=$!

echo "等待5秒..."
sleep 5

echo "停止进程..."
kill $COLLECTOR_PID $GENERATOR_PID 2>/dev/null

echo "====== Generator Logs ======"
head -50 generator.log

echo ""
echo "====== Collector Logs ======"
head -100 collector.log | grep -E "(ERROR|WARN|Successfully|Failed to deserialize|Received message|After trimming)"

echo ""
echo "测试完成"