//! Buddy 修改：`mermaid` 模块 **stub**
//!
//! # 为什么
//!
//! zed 的 `crates/markdown/src/mermaid.rs`（1836 行）依赖 `mermaid_render`
//! （zed 内的 path 依赖，含 node/wasm 渲染链路）。Buddy **不需要 mermaid**，
//! 而 `markdown.rs` 里有 **90 处**引用该模块，删不干净。
//!
//! # 做法
//!
//! 提供**同样 API 表面**的空实现，并把 `MarkdownOptions::render_mermaid_diagrams`
//! 默认置为 `false` —— 这样 `render_mermaid_diagram()` 在运行时**永不被调用**，
//! 只需通过编译即可。
//!
//! 含 mermaid 代码块将按普通代码块渲染。

use std::collections::BTreeMap;
use std::ops::Range;

use gpui::{AnyElement, App, Entity, SharedString, Window};
// 用 parser 的别名，保证生命周期与调用点一致（直接 use pulldown_cmark::Event 会不一致）
use crate::parser::MarkdownEvent;

use crate::{CopyButtonVisibility, Markdown, MarkdownStyle, MermaidZoomCallback};

/// 与 zed 同名同字段
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ParsedMarkdownMermaidDiagramContents {
    pub(crate) contents: SharedString,
    pub(crate) scale: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct ParsedMarkdownMermaidDiagram {
    pub(crate) content_range: Range<usize>,
    pub(crate) contents: ParsedMarkdownMermaidDiagramContents,
}

#[derive(Default, Clone)]
pub(crate) struct MermaidState;

impl MermaidState {
    /// 空实现：无缓存可清
    pub(crate) fn clear(&mut self, _cx: &mut App) {}

    /// 空实现：`mermaid_diagrams` 恒为空，无需更新渲染任务
    pub(crate) fn update(
        &mut self,
        _parsed: &crate::ParsedMarkdown,
        _zoom_for: impl Fn(usize) -> f32,
        _cx: &mut App,
    ) {
    }

    /// 恒返回 `None` —— 因为图表永远不存在，调用方会走 `return 1.0` 分支
    pub(crate) fn natural_size(
        &self,
        _contents: &ParsedMarkdownMermaidDiagramContents,
    ) -> Option<gpui::Size<gpui::Pixels>> {
        None
    }

    /// 空实现
    pub(crate) fn rerasterize_diagram(
        &mut self,
        _contents: &ParsedMarkdownMermaidDiagramContents,
        _zoom: f32,
        _cx: &mut App,
    ) {
    }
}

/// **空实现**：返回空映射 → `parsed_markdown.mermaid_diagrams` 恒为空
pub(crate) fn extract_mermaid_diagrams(
    _source: &str,
    _events: &[(Range<usize>, MarkdownEvent)],
) -> BTreeMap<usize, ParsedMarkdownMermaidDiagram> {
    BTreeMap::default()
}

/// **永不被调用**（因为 `render_mermaid_diagrams` 为 false）。
/// 保留完整签名以确保 vendored `markdown.rs` 无需改动调用点。
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_mermaid_diagram(
    _parsed: &ParsedMarkdownMermaidDiagram,
    _mermaid_state: &MermaidState,
    _style: &MarkdownStyle,
    _markdown: Entity<Markdown>,
    _source_offset: usize,
    _showing_code: bool,
    _zoom: f32,
    _copy_button_visibility: CopyButtonVisibility,
    _on_zoom: Option<MermaidZoomCallback>,
    _window: &mut Window,
    _cx: &mut App,
) -> AnyElement {
    // 不会到达这里：`MarkdownOptions::render_mermaid_diagrams` 默认 false，
    // 且所有入口都先检查该标志。
    unreachable!("mermaid 已被裁掉，不应渲染 mermaid 图表")
}
