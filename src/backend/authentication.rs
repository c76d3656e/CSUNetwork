use std::time::Duration;
use std::sync::Arc;
use tokio::runtime::Runtime;
use std::process::Command;
use std::process::Stdio;
use thirtyfour::prelude::*;
use anyhow::{Result, anyhow};
use log::info;
use crate::backend::config::{Config, ISP};
use crate::backend::network_monitor::NetworkMonitor;

/// 认证器状态结构体
#[derive(Default)]
struct DriverState {
    driver: Option<WebDriver>,
    chromedriver_process: Option<std::process::Child>,
}

/// 认证器结构体
pub struct Authenticator {
    config: Arc<Config>,
    driver_state: DriverState,
    network_monitor: NetworkMonitor,
}

impl Authenticator {
    /// 创建新的认证器实例
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            driver_state: DriverState::default(),
            network_monitor: NetworkMonitor::new(),
        }
    }

    /// 初始化认证器
    pub async fn init(&mut self) -> Result<()> {
        // 检查 ChromeDriver 是否存在
        let current_dir = std::env::current_dir()?;
        let chromedriver_path = current_dir.join("chromedriver.exe");

        if !chromedriver_path.exists() {
            return Err(anyhow!("ChromeDriver not found at: {}", chromedriver_path.display()));
        }

        // 尝试启动 ChromeDriver
        if let Err(e) = self.start_chromedriver() {
            return Err(anyhow!("Failed to start ChromeDriver: {}", e));
        }

        // 尝试创建 WebDriver
        match self.create_webdriver().await {
            Ok(driver) => {
                self.driver_state.driver = Some(driver);
                Ok(())
            }
            Err(e) => {
                // 如果创建 WebDriver 失败，确保关闭 ChromeDriver
                if let Some(mut process) = self.driver_state.chromedriver_process.take() {
                    let _ = process.kill();
                }
                Err(anyhow!("Failed to create WebDriver: {}", e))
            }
        }
    }

    /// 启动 ChromeDriver
    fn start_chromedriver(&mut self) -> Result<()> {
        // 先检查 ChromeDriver 是否已在运行
        if let Some(p) = &mut self.driver_state.chromedriver_process {
            match p.try_wait() {
                Ok(Some(_)) => {
                    self.driver_state.chromedriver_process = None;
                }
                Ok(None) => {
                    return Ok(());
                }
                Err(_) => {
                    self.driver_state.chromedriver_process = None;
                }
            }
        }

        let current_dir = std::env::current_dir()?;
        let chromedriver_path = current_dir.join("chromedriver.exe");

        info!("Starting ChromeDriver...");
        let child = Command::new(chromedriver_path)
            .arg("--port=9515")
            .spawn()?;

        self.driver_state.chromedriver_process = Some(child);
        
        // 等待 ChromeDriver 启动
        std::thread::sleep(Duration::from_secs(2));
        
        Ok(())
    }

    /// 创建 WebDriver
    async fn create_webdriver(&mut self) -> Result<WebDriver> {
        let mut caps = DesiredCapabilities::chrome();
        
        // 配置 Chrome 选项
        let chrome_args = vec![
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--ignore-certificate-errors",
        ];

        for arg in chrome_args {
            caps.add_chrome_arg(arg)?;
        }

        // 设置 Chrome 路径
        let chrome_paths = vec![
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            "./chrome-win32/chrome.exe",  // 相对于当前目录的路径
            "./chrome-win64/chrome.exe",  // 相对于当前目录的路径
        ];

        let mut chrome_found = false;
        for path in chrome_paths {
            if std::path::Path::new(path).exists() {
                info!("Found Chrome at: {}", path);
                caps.set_binary(path)?;
                chrome_found = true;
                break;
            }
        }

        if !chrome_found {
            return Err(anyhow!("Chrome browser not found. Please install Chrome or specify its location."));
        }

        // 设置超时和其他选项
        caps.add_chrome_arg("--start-maximized")?;  // 最大化窗口
        caps.add_chrome_arg("--disable-extensions")?;  // 禁用扩展
        caps.add_chrome_arg("--disable-popup-blocking")?;  // 禁用弹窗阻止
        caps.add_chrome_arg("--disable-infobars")?;  // 禁用信息栏

        info!("Creating WebDriver with configured capabilities...");
        let driver = WebDriver::new("http://localhost:9515", caps).await?;
        
        // 设置超时
        driver.set_page_load_timeout(Duration::from_secs(30)).await?;
        driver.set_script_timeout(Duration::from_secs(30)).await?;
        driver.set_implicit_wait_timeout(Duration::from_secs(10)).await?;
        
        Ok(driver)
    }

    /// 打开认证页面
    pub async fn open_auth_page(&mut self) -> Result<()> {
        if let Some(driver) = &self.driver_state.driver {
            info!("Navigating to login page...");
            driver.goto(&self.config.auth_url).await?;
            Ok(())
        } else {
            Err(anyhow!("WebDriver not initialized"))
        }
    }

    /// 执行登录操作
    /// 账号的js路径 document.querySelector("#login-box > div > div.mt_body > div:nth-child(1) > div > form > input:nth-child(2)")
    /// 密码的js路径 document.querySelector("#login-box > div > div.mt_body > div:nth-child(1) > div > form > input:nth-child(3)")
    /// 运营商的xpath路径 //*[@id="login-box"]/div/div[3]/div[1]/div/select
    /// 运营商的值 移动“@cmccn” 联通“@unicomn” 电信“@telecomn” 校园网“”
    /// 登录按钮的js路径 document.querySelector("#login-box > div > div.mt_body > div:nth-child(1) > div > form > input.edit_lobo_cell.sms_login")
    pub async fn login(&mut self) -> Result<()> {
        self.init().await?;
        let driver = self.driver_state.driver.as_ref()
            .ok_or_else(|| anyhow!("WebDriver not initialized"))?;
        
        driver.goto(&self.config.auth_url).await?;
        info!("Filling login form...");
        
        // 等待页面加载完成
        std::thread::sleep(Duration::from_secs(3));
        
        // 输入用户名
        let username_input = driver.query(By::Css("#login-box > div > div.mt_body > div:nth-child(1) > div > form > input:nth-child(2)"))
            .wait(Duration::from_secs(10), Duration::from_millis(500))
            .first()
            .await?;
        username_input.send_keys(&self.config.username).await?;
        
        // 输入密码
        let password_input = driver.query(By::Css("#login-box > div > div.mt_body > div:nth-child(1) > div > form > input:nth-child(3)"))
            .wait(Duration::from_secs(10), Duration::from_millis(500))
            .first()
            .await?;
        password_input.send_keys(&self.config.password).await?;     
        
         // 使用 XPath 定位 <select> 元素
        let isp_select = driver.query(By::XPath("//*[@id='login-box']/div/div[3]/div[1]/div/select"))
            .wait(Duration::from_secs(10), Duration::from_millis(500))
            .first()
            .await?;

        // 点击 <select> 元素展开选项
        isp_select.click().await?;

        // 根据配置选择目标 <option> 元素
        let isp_value = match self.config.isp {
            ISP::Mobile => "@cmccn",
            ISP::Unicom => "@unicomn",
            ISP::Telecom => "@telecomn",
            ISP::School => "",
        };

        // 使用 XPath 定位目标 <option> 元素并点击
        let target_option = driver.query(By::XPath(&format!("//*[@id='login-box']/div/div[3]/div[1]/div/select/option[@value='{}']", isp_value)))
            .wait(Duration::from_secs(10), Duration::from_millis(500))
            .first()
            .await?;
        target_option.click().await?;

        // 点击登录按钮
        let login_button = driver.query(By::Css("#login-box > div > div.mt_body > div:nth-child(1) > div > form > input.edit_lobo_cell.sms_login"))
            .wait(Duration::from_secs(10), Duration::from_millis(500))
            .first()
            .await?;
        login_button.click().await?;

        info!("Login button clicked, waiting for network to be ready...");
        
        // 等待登录完成和网络就绪
        std::thread::sleep(Duration::from_secs(3));
        
        // 检查登录是否成功
        if let Ok(current_url) = driver.current_url().await {
            if current_url.as_str() != self.config.auth_url {
                info!("Login successful, redirected to: {}", current_url.as_str());
            } else {
                return Err(anyhow!("Login failed: Still on login page"));
            }
        }
        
        self.quit().await?;
        Ok(())
    }

    /// 执行登出操作
    pub async fn logout(&mut self) -> Result<()> {
        self.init().await?;
        // 循环两次才能登出
        for _ in 0..2 {

        let driver = self.driver_state.driver.as_ref()
            .ok_or_else(|| anyhow!("WebDriver not initialized"))?;
        driver.goto(&self.config.auth_url).await?;
        info!("Executing logout...");
        
        // 等待页面加载完成
        std::thread::sleep(Duration::from_secs(3));
        
        // 使用 JavaScript 点击登出按钮
        let logout_script = r#"
            function clickLogout() {
                var button = document.querySelector('#edit_body > div > div.edit_loginBox.ui-resizable-autohide > form > input');
                if (!button) {
                    javascript:wc();
                    return true;
                }
                button.click();
                return true;
            }
            return clickLogout();
        "#;
        
        driver.execute(logout_script, Vec::new()).await?;
        
        // 等待确认对话框出现
        std::thread::sleep(Duration::from_secs(2));
        
        // 点击确认按钮
        let confirm_script = r#"
            function clickConfirm() {
                var button = document.querySelector('#layui-layer1 > div.layui-layer-btn.layui-layer-btn- > a.layui-layer-btn0');
                if (!button) {
                    return false;
                }
                button.click();
                return true;
            }
            return clickConfirm();
        "#;
        
        driver.execute(confirm_script, Vec::new()).await?;
        
        // 等待登出完成
        // std::thread::sleep(Duration::from_secs(5));
        }
        // 等待登出完成
        std::thread::sleep(Duration::from_secs(3));
        self.quit().await?;
        Ok(())
    }

    /// 关闭浏览器和清理资源
    pub async fn quit(&mut self) -> Result<()> {
        if let Some(driver) = self.driver_state.driver.take() {
            info!("Closing browser...");
            driver.quit().await?;
        }
        
        if let Some(mut process) = self.driver_state.chromedriver_process.take() {
            info!("Stopping ChromeDriver...");
            let _ = process.kill();
        }
        
        Ok(())
    }
}

impl Drop for Authenticator {
    fn drop(&mut self) {
        if let Some(mut process) = self.driver_state.chromedriver_process.take() {
            let _ = process.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    /// 创建测试配置
    fn create_test_config() -> Arc<Config> {
        Arc::new(Config {
            username: "test_user".to_string(),
            password: "test_pass".to_string(),
            auth_url: "http://10.1.1.1".to_string(),
            isp: ISP::School,
            remember_password: true,
            auto_login: false,
        })
    }

    #[tokio::test]
    async fn test_authenticator_creation() {
        let config = create_test_config();
        let auth = Authenticator::new(config);
        assert!(auth.driver_state.driver.is_none());
        assert!(auth.driver_state.chromedriver_process.is_none());
    }

    #[tokio::test]
    async fn test_authenticator_initialization() {
        let config = create_test_config();
        let mut auth = Authenticator::new(config);

        let result = auth.init().await;
        // 由于测试环境中可能没有 ChromeDriver，所以初始化可能失败
        if let Err(e) = &result {
            println!("ChromeDriver initialization failed as expected: {}", e);
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("ChromeDriver not found") || 
                error_msg.contains("Failed to start ChromeDriver") ||
                error_msg.contains("cannot find Chrome binary") ||
                error_msg.contains("tcp connect error") ||
                error_msg.contains("webdriver server did not respond") ||
                error_msg.contains("Chrome browser not found"),
                "Unexpected error message: {}", error_msg
            );
        }
    }

    #[tokio::test]
    async fn test_login_process() {
        let config = create_test_config();
        let mut auth = Authenticator::new(config);

        // 尝试在未初始化的情况下登录
        let result = auth.login().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("ChromeDriver not found"));

        // 初始化认证器（预期会失败，因为没有 ChromeDriver）
        let init_result = auth.init().await;
        assert!(init_result.is_err());
    }

    #[tokio::test]
    async fn test_logout_process() {
        let config = create_test_config();
        let mut auth = Authenticator::new(config);

        // 尝试在未初始化的情况下登出
        let result = auth.logout().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("ChromeDriver not found"));

        // 初始化认证器（预期会失败，因为没有 ChromeDriver）
        let init_result = auth.init().await;
        assert!(init_result.is_err());
    }
} 