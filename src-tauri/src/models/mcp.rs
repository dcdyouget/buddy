// ============================================================================
// MCP server 配置模型
// ============================================================================
//
// 配置项来源：config.json 的 `mcp_servers` 数组。
// 字段定义采用 `#[serde(default)]` 保证老配置无相关字段时也能加载。
//
// 设计原则：
// - id 由前端在新建时生成 UUID，backend 只读
// - env 不支持系统变量展开（按 P3 决策），但 stdio 进程默认继承父进程环境
// - API key 明文存（按 P3 决策），headers 可携带任意键值对
// - timeout / reconnect 必填（带默认值，符合 "可设置但要设置默认值"）
// - stdio shutdown 由 McpClient 控制 graceful SIGTERM
// ============================================================================

use serde::{Deserialize, Serialize};

/// MCP server 传输方式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    /// 本地子进程，stdin/stdout 通信
    Stdio,
    /// 远程 HTTP + SSE
    Sse,
}

/// 单个 MCP server 配置
///
/// 字段均带 `#[serde(default)]`，老配置缺字段时使用默认值。
/// `id` 必须存在——它是 server 标识，缺了视为不合法配置（前端负责生成）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// UUID v4 唯一标识，前端在新建时生成
    pub id: String,
    /// 用户可读的展示名（如"本地文件系统"）
    pub name: String,
    /// 是否启用；false 时不参与 tool 聚合
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 传输方式
    pub transport: McpTransport,

    // ── stdio 字段 ──
    /// 启动命令（如 "npx" / "node" / "/usr/local/bin/mcp-fs"）
    /// 仅 stdio 模式使用
    #[serde(default)]
    pub command: Option<String>,
    /// 命令参数数组
    #[serde(default)]
    pub args: Vec<String>,
    /// 注入到子进程的环境变量；不填则继承父进程环境
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,

    // ── sse/http 字段 ──
    /// SSE 端点 URL
    /// 仅 sse 模式使用
    #[serde(default)]
    pub url: Option<String>,
    /// HTTP 请求头（用于鉴权等）；value 明文存
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,

    // ── 通用可配字段（带默认值）──
    /// 请求超时（秒），默认 30
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u32,
    /// 异常断开后是否自动重连，仅对 stdio 有效（HTTP/SSE 由 client 自行处理）
    /// 默认 true
    #[serde(default = "default_true")]
    pub auto_reconnect: bool,
}

fn default_true() -> bool {
    true
}

fn default_timeout_secs() -> u32 {
    30
}

impl McpServerConfig {
    /// stdio 配置是否完整（必填字段都填了）
    #[allow(dead_code)] // 公共校验 API，前端配置面板调用
    pub fn validate_stdio(&self) -> Result<(), String> {
        if self.command.as_deref().unwrap_or("").is_empty() {
            return Err("stdio 模式必须填写 command".to_string());
        }
        Ok(())
    }

    /// sse 配置是否完整
    #[allow(dead_code)] // 公共校验 API，前端配置面板调用
    pub fn validate_sse(&self) -> Result<(), String> {
        if self.url.as_deref().unwrap_or("").is_empty() {
            return Err("sse 模式必须填写 url".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let json = r#"{
            "id": "test-1",
            "name": "test",
            "transport": "stdio",
            "command": "npx"
        }"#;
        let cfg: McpServerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.enabled, true);
        assert_eq!(cfg.timeout_secs, 30);
        assert_eq!(cfg.auto_reconnect, true);
        assert_eq!(cfg.args.len(), 0);
        assert!(cfg.command.as_deref().unwrap() == "npx");
    }

    #[test]
    fn test_stdio_validation() {
        let mut cfg = McpServerConfig {
            id: "x".into(),
            name: "x".into(),
            enabled: true,
            transport: McpTransport::Stdio,
            command: None,
            args: vec![],
            env: Default::default(),
            url: None,
            headers: Default::default(),
            timeout_secs: 30,
            auto_reconnect: true,
        };
        assert!(cfg.validate_stdio().is_err());
        cfg.command = Some("npx".into());
        assert!(cfg.validate_stdio().is_ok());
    }

    #[test]
    fn test_sse_validation() {
        let mut cfg = McpServerConfig {
            id: "x".into(),
            name: "x".into(),
            enabled: true,
            transport: McpTransport::Sse,
            command: None,
            args: vec![],
            env: Default::default(),
            url: None,
            headers: Default::default(),
            timeout_secs: 30,
            auto_reconnect: true,
        };
        assert!(cfg.validate_sse().is_err());
        cfg.url = Some("https://example.com/sse".into());
        assert!(cfg.validate_sse().is_ok());
    }
}
