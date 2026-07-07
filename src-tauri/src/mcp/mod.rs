// ============================================================================
// MCP (Model Context Protocol) 客户端模块
// ============================================================================
//
// 职责:
// - JSON-RPC 2.0 over stdio / HTTP+SSE
// - 启动时与每个配置的 server 握手(initialize),拉取 tool 列表
// - 把 MCP tool 包装成 Arc<dyn Tool> 注入 ToolRegistry
// - 异常断开时按 auto_reconnect 决定是否重启(stdio)
// - graceful shutdown:SIGTERM 等 5s 再 SIGKILL
//
// P1 阶段:仅占位,实际实现分布在 P6 (传输层) 和 P7 (JSON-RPC + 工具发现)
//
// 模块结构(P6-P7 完成后):
//   mcp/
//   ├── mod.rs        (本文件,入口 + re-export)
//   ├── config.rs     (McpServerConfig 已在 models::mcp 中,本处 re-export)
//   ├── transport.rs  (McpTransport trait + Stdio/Sse 实现)
//   ├── stdio.rs      (Stdio 传输)
//   ├── sse.rs        (SSE 传输)
//   ├── protocol.rs   (JSON-RPC 2.0 帧 + MCP 协议方法)
//   └── client.rs     (McpClient,聚合 transport + protocol,提供 list_tools/call_tool)
//
// 当前为空,后续阶段会逐文件实现。
// ============================================================================

// P6 会:
//   pub mod transport;
//   pub mod stdio;
//   pub mod sse;
//
// P7 会:
//   pub mod protocol;
//   pub mod client;
//
// 现在 re-export 配置(让外部统一从 mcp::McpServerConfig 引用)
// P6-P7 完成传输层后会被实际代码使用
#[allow(unused_imports)]
pub use crate::models::mcp::{McpServerConfig, McpTransport};
