use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;

/// 认证响应的JSON结构
#[derive(Debug, Deserialize)]
pub struct AuthResponse {
    pub result: i32,
    pub msg: String,
    pub ret_code: i32,
}

/// 运营商类型
#[derive(Debug, Clone)]
pub enum ISP {
    Unicom,    // 联通 @unicomn
    Mobile,    // 移动 @cmccn
    Telecom,   // 电信 @telecomn
    Campus,    // 校园网 ""
}

impl ISP {
    fn as_str(&self) -> &'static str {
        match self {
            ISP::Unicom => "unicomn",
            ISP::Mobile => "cmccn",
            ISP::Telecom => "telecomn",
            ISP::Campus => "",
        }
    }
}

/// 认证客户端结构
pub struct AuthClient {
    client: Client,
    base_url: String,
    username: String,
    password: String,
    isp: ISP,
}

impl AuthClient {
    /// 创建新的认证客户端实例
    pub fn new(username: String, password: String, isp: ISP) -> Self {
        Self {
            client: Client::builder()
                .danger_accept_invalid_certs(true)  // 接受无效证书
                .build()
                .unwrap_or_else(|_| Client::new()),
            base_url: "https://portal.csu.edu.cn:802/eportal/portal".to_string(),
            username,
            password,
            isp,
        }
    }

    /// 从响应文本中提取IP地址
    fn extract_ip(text: &str) -> Option<String> {
        // 按优先级尝试不同的IP提取方法
        if text.contains("v46ip") {
            if let Some(ip) = text.split("v46ip='").nth(1).and_then(|s| s.split('\'').next()) {
                return Some(ip.to_string());
            }
        }
        
        if text.contains("v4ip") {
            if let Some(ip) = text.split("v4ip='").nth(1).and_then(|s| s.split('\'').next()) {
                return Some(ip.to_string());
            }
        }
        
        if text.contains("ss5") {
            if let Some(ip) = text.split("ss5=\"").nth(1).and_then(|s| s.split('\"').next()) {
                return Some(ip.to_string());
            }
        }
        
        None
    }

    /// 获取IP地址
    pub async fn get_ip(&self) -> Result<String, Box<dyn Error>> {
        let response = self.client
            .get("http://10.1.1.1")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0")
            .send()
            .await?;
            
        let text = response.text().await?;
        
        if let Some(ip) = Self::extract_ip(&text) {
            Ok(ip)
        } else {
            Err("无法获取IP地址".into())
        }
    }

    /// 执行登录请求
    pub async fn login(&self) -> Result<AuthResponse, Box<dyn Error>> {
        // 获取IP地址
        let ip = self.get_ip().await?;
        
        // 构造用户账号
        let user_account = format!(",1,{}@{}", self.username, self.isp.as_str());
        
        // 构造请求参数
        let mut params = HashMap::new();
        let callback = "dr1004".to_string();
        let login_method = "1".to_string();
        
        params.insert("callback", &callback);
        params.insert("login_method", &login_method);
        params.insert("user_account", &user_account);
        params.insert("user_password", &self.password);
        params.insert("wlan_user_ip", &ip);

        // 发送请求
        let response = self
            .client
            .get(&format!("{}/login", self.base_url))
            .query(&params)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0")
            .header("Referer", "https://portal.csu.edu.cn/")
            .header("Origin", "https://portal.csu.edu.cn")
            .send()
            .await?;

        // 获取响应文本
        let text = response.text().await?;
        
        // 解析JSONP响应
        let json_str = text
            .trim_start_matches("dr1004(")
            .trim_end_matches(");");
            
        // 解析JSON
        let auth_response: AuthResponse = serde_json::from_str(json_str)?;
        
        Ok(auth_response)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;
    #[tokio::test]
    async fn test_auth_flow() {
        let client = AuthClient::new(
            "1234567890".to_string(),
            "1234567890".to_string(),
            ISP::Unicom,
        );
        match client.login().await {
            Ok(response) => println!("登录结果: {:?}", response),
            Err(e) => println!("登录失败: {}", e),
        }
    }
}
