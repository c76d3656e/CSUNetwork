// 配置管理模块
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use anyhow::Result;
use log::info;

// 运营商枚举
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ISP {
    Mobile,
    Unicom,
    Telecom,
    School,
}

impl Default for ISP {
    fn default() -> Self {
        ISP::School
    }
}

// 配置文件结构
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Config {
    pub username: String,
    pub password: String,
    pub remember_password: bool,
    pub auto_login: bool,
    pub auth_url: String,
    pub isp: ISP,
}

impl Config {
    // 获取配置文件路径
    fn get_config_path() -> PathBuf {
        let mut path = PathBuf::from("config");
        path.push("config.json");
        path
    }

    // 加载配置
    pub fn load() -> Result<Self> {
        let path = Self::get_config_path();
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let mut config: Config = serde_json::from_str(&content)?;
            
            // 如果认证URL为空，设置默认值
            if config.auth_url.is_empty() {
                config.auth_url = "http://10.1.1.1".to_string();
            }
            
            // 如果不记住密码，确保密码被清空
            if !config.remember_password {
                config.password = String::new();
                config.auto_login = false;
            }
            
            info!("Configuration loaded successfully from {:?}", path);
            Ok(config)
        } else {
            info!("No configuration file found at {:?}, using defaults", path);
            Ok(Config {
                auth_url: "http://10.1.1.1".to_string(),
                ..Default::default()
            })
        }
    }

    // 保存配置
    pub fn save(&self) -> Result<()> {
        let path = Self::get_config_path();
        
        // 确保配置目录存在
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // 如果不记住密码，则清空密码再保存
        let mut config_to_save = self.clone();
        if !self.remember_password {
            config_to_save.password = String::new();
            config_to_save.auto_login = false;
        }

        let content = serde_json::to_string_pretty(&config_to_save)?;
        fs::write(&path, content)?;
        info!("Configuration saved successfully to {:?}", path);
        Ok(())
    }

    // 用于测试的直接保存和加载方法
    #[cfg(test)]
    fn save_to(&self, path: &PathBuf) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // 如果不记住密码，则清空密码再保存
        let mut config_to_save = self.clone();
        if !self.remember_password {
            config_to_save.password = String::new();
            config_to_save.auto_login = false;
        }

        let content = serde_json::to_string_pretty(&config_to_save)?;
        fs::write(path, content)?;
        Ok(())
    }

    #[cfg(test)]
    fn load_from(path: &PathBuf) -> Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let config = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config {
                auth_url: "http://10.1.1.1".to_string(),
                ..Default::default()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_save_load() {
        let test_dir = env::current_dir().unwrap().join("test_config");
        fs::create_dir_all(&test_dir).unwrap();
        let config_path = test_dir.join("config.json");

        let config = Config {
            username: "test_user".to_string(),
            password: "test_pass".to_string(),
            remember_password: true,
            auto_login: true,
            auth_url: "http://10.1.1.1".to_string(),
            isp: ISP::School,
        };

        // 保存配置
        config.save_to(&config_path).unwrap();

        // 读取配置
        let loaded_config = Config::load_from(&config_path).unwrap();

        // 因为remember_password为true，所有字段都应该保持不变
        assert_eq!(config.username, loaded_config.username);
        assert_eq!(config.password, loaded_config.password);
        assert_eq!(config.remember_password, loaded_config.remember_password);
        assert_eq!(config.auto_login, loaded_config.auto_login);
        assert_eq!(config.auth_url, loaded_config.auth_url);
        assert_eq!(config.isp, loaded_config.isp);

        fs::remove_dir_all(test_dir).unwrap_or_default();
    }

    #[test]
    fn test_config_no_remember() {
        let test_dir = env::current_dir().unwrap().join("test_config_no_remember");
        fs::create_dir_all(&test_dir).unwrap();
        let config_path = test_dir.join("config.json");

        let config = Config {
            username: "test_user".to_string(),
            password: "test_pass".to_string(),
            remember_password: false,
            auto_login: false,
            auth_url: "http://10.1.1.1".to_string(),
            isp: ISP::Mobile,
        };

        // 保存配置
        config.save_to(&config_path).unwrap();

        // 读取配置
        let loaded_config = Config::load_from(&config_path).unwrap();

        // 验证结果
        assert_eq!(config.username, loaded_config.username);
        assert!(loaded_config.password.is_empty()); // 密码应该被清空
        assert!(!loaded_config.remember_password);
        assert!(!loaded_config.auto_login);
        assert_eq!(config.auth_url, loaded_config.auth_url);
        assert_eq!(config.isp, loaded_config.isp);

        fs::remove_dir_all(test_dir).unwrap_or_default();
    }
} 