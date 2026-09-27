//! zed `util` crate 的最小替身（S04-03）。
//!
//! vendored markdown 只用到 `util` 的三项：
//! - `util::maybe!`、`util::ResultExt` —— zed `util` 本身就是从 `gpui_util` 重导出的（`util.rs:43`
//!   `pub use gpui_util::*;`），而 `gpui_util` 已是 GPUI 的依赖 → 这里同样重导出，零新增依赖；
//! - `util::markdown::generate_heading_slug` —— 下方逐字复制自 zed rev `290cbcb`
//!   `crates/util/src/markdown.rs:4-17`（Apache-2.0）。
//!
//! 为什么不直接依赖 zed `util`：它另外拖入 `async_zip` / `rust-embed` / `nix` / `globset` 等 36 个包（S04-01 实测），
//! 而且在 workspace 中只有 buddy-markdown 使用它（`cargo tree -i util`）。

pub use gpui_util::*;

/// 标题锚点 slug（与 zed 实现逐字一致）
pub mod markdown {
    // Copied verbatim from zed-industries/zed rev 290cbcb9cb6a5dcbe0060431a126ad19e743f2f4,
    // crates/util/src/markdown.rs, licensed under Apache-2.0.
    pub fn generate_heading_slug(text: &str) -> String {
        text.trim()
            .chars()
            .filter_map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    Some(c.to_lowercase().next().unwrap_or(c))
                } else if c == ' ' {
                    Some('-')
                } else {
                    None
                }
            })
            .collect()
    }
}
