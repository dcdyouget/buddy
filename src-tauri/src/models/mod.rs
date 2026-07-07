// ============================================================================
// 数据模型模块入口（mod.rs）
// ============================================================================
//
// Rust 模块系统说明：
// - 一个文件夹 + 一个 mod.rs = 一个模块（这是 2015 edition 风格；2018 起也允许
//   直接用文件名作为模块入口）
// - 声明子模块（pub mod config;）= 在当前模块下声明并对外开放一个名为 config 的子模块
// - 子模块的源码文件位于同级目录的 `config.rs` 中
//
// 对应到 Java：
// - 一个 mod.rs ≈ 一个 package-info.java + 该 package 下的类文件集合
// - `pub mod config;` ≈ 在 package 中存在一个 `Config.java`
// ============================================================================

pub mod config;            // 声明并公开子模块 config（位于 ./config.rs）
pub mod message;           // 声明并公开子模块 message（位于 ./message.rs）
pub mod model_context;     // 声明并公开子模块 model_context（位于 ./model_context.rs）
pub mod storage;           // 声明并公开子模块 storage（位于 ./storage.rs）
pub mod mcp;               // MCP server 配置（位于 ./mcp.rs）

// ============================================================================
// re-export（重导出）
// ============================================================================
//
// `pub use config::*;` 会把子模块的所有 pub 项"提到"当前模块来。
// 效果：调用方既可以写 `crate::models::message::Message`，
//      也可以直接写 `crate::models::Message`（同一个东西）
//
// Rust 语法小知识：
// - `use A::B;`   = Java 的 `import A.B;`
// - `pub use`     = 既做 use，又把它对外公开（Java 默认 import 后是 public 的）
// - `*`（glob）   = Java 没有 glob import；这里表示"子模块的全部 pub 项"
//
// 为什么这么做？让外部少写一层路径，同时保持文件物理组织。
// ============================================================================

pub use config::*;         // 把 config 子模块的 pub 项全部 re-export
pub use message::*;        // 把 message 子模块的 pub 项全部 re-export
pub use model_context::*;  // 把 model_context 子模块的 pub 项全部 re-export
pub use storage::*;        // 把 storage 子模块的 pub 项全部 re-export