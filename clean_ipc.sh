#!/bin/bash

echo "清理IceOryx2 IPC资源..."

# 杀死所有相关进程
echo "停止所有相关进程..."
pkill -f "signal-collector" 2>/dev/null
pkill -f "ice-signal-generator" 2>/dev/null
pkill -f "test-runner" 2>/dev/null

# 等待进程完全停止
sleep 1

# IceOryx2 使用的是不同的命名模式
# 清理共享内存段
echo "清理共享内存..."
rm -rf /dev/shm/org.eclipse.iceoryx2.* 2>/dev/null
rm -rf /dev/shm/iox2_* 2>/dev/null
rm -rf /dev/shm/*iceoryx* 2>/dev/null

# 清理临时文件和锁文件
echo "清理临时文件..."
rm -rf /tmp/org.eclipse.iceoryx2.* 2>/dev/null
rm -rf /tmp/iox2_* 2>/dev/null
rm -rf /tmp/*iceoryx* 2>/dev/null

# 如果在 macOS 上，还需要清理其他位置
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "清理macOS特定位置..."
    rm -rf ~/Library/Caches/org.eclipse.iceoryx2.* 2>/dev/null
    rm -rf /var/folders/*/*/T/org.eclipse.iceoryx2.* 2>/dev/null
    rm -rf /var/folders/*/*/T/iox2_* 2>/dev/null
    
    # 清理可能的配置文件
    rm -rf ~/.config/iceoryx2 2>/dev/null
    rm -rf ~/.local/share/iceoryx2 2>/dev/null
fi

# 清理可能的socket文件
rm -rf /var/run/iox* 2>/dev/null
rm -rf /tmp/iox* 2>/dev/null

echo "IceOryx2资源清理完成！"
echo "现在可以重新启动测试了。"