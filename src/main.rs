use std::sync::Arc;
use log::{info, error};
use crate::frontend::ui::UI;
use crate::backend::network_monitor::NetworkMonitor;
use crate::backend::logger::Logger;

mod frontend;
mod backend;

#[tokio::main]
async fn main() {
    // 初始化日志系统
    if let Err(e) = Logger::init() {
        eprintln!("Failed to initialize logger: {}", e);
        std::process::exit(1);
    }
    info!("Starting Campus Network Assistant...");

    // 创建网络监控器
    let network_monitor = Arc::new(NetworkMonitor::new());
    
    // 创建并运行UI
    let ui = UI::new(network_monitor);
    if let Err(e) = ui.run() {
        error!("UI error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_network_monitor_initialization() {
        let network_monitor = Arc::new(NetworkMonitor::new());
        assert!(!network_monitor.is_connected());
    }

    #[tokio::test]
    async fn test_network_monitor_connection_check() {
        let network_monitor = Arc::new(NetworkMonitor::new());
        network_monitor.check_connection().await;
        // Note: This test depends on actual network connection
    }

    #[test]
    fn test_ui_initialization() {
        let network_monitor = Arc::new(NetworkMonitor::new());
        let ui = UI::new_empty(network_monitor);
        // Test UI initial state
        assert!(ui.config.username.is_empty());
        assert!(ui.config.password.is_empty());
        assert!(!ui.config.remember_password);
        assert!(!ui.config.auto_login);
    }

    #[test]
    fn test_environment_setup() {
        std::env::set_var("RUST_LOG", "info");
        assert_eq!(std::env::var("RUST_LOG").unwrap(), "info");
    }
}
