use anyhow::Result;
use std::process::Command;
use std::time::Duration;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    info!("启动IceOryx2测试环境");
    
    // 启动信号收集器
    let mut signal_collector = Command::new("cargo")
        .args(&["run", "--bin", "signal-collector"])
        .current_dir("../signal-collector")
        .spawn()?;
    
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // 启动IceOryx信号生成器
    let mut ice_generator = Command::new("cargo")
        .args(&["run", "--bin", "ice-signal-generator"])
        .spawn()?;
    
    info!("IceOryx2测试进程已启动");
    info!("信号收集器和信号生成器正在运行");
    info!("按 Ctrl+C 停止测试");
    
    // 等待中断信号
    tokio::signal::ctrl_c().await?;
    
    info!("停止所有进程");
    
    // 终止所有进程
    let _ = signal_collector.kill();
    let _ = ice_generator.kill();
    
    Ok(())
}