//! 网络搜索卡片（S05-11）—— 对应 v1 `WebSearchSection.tsx` 与 `global.css` 的 `.websearch-*`（外壳沿用 `.think-section`）
//!
//! | v1 | 本模块 |
//! |----|------|
//! | 工具名 `websearch` 走专用卡片，不走通用工具卡 | 由消息列表分派（[`is_web_search`]） |
//! | 标题栏：搜索图标 + 状态文案（正在搜索网络 / 网络搜索完成 / 网络搜索不可用，已继续回答 / 网络搜索已中断）+ 搜索中的三点加载与光泽 + 「· 关键词」 + 折叠箭头；默认折叠 | [`block`]、[`label`] |
//! | 展开：「搜索内容 / 搜索引擎」两行、可选说明、「搜索结果（N）」列表；搜索中「正在等待搜索结果…」；无结果「没有可展示的搜索结果 / 搜索未返回结果」 | 同，最高 336px 内部滚动 |
//! | 结果项：序号圆标 + 来源胶囊 + 标题链接（点击用系统浏览器打开）+ 最多 3 行摘要 + 正文读取状态（已读取网页正文 / 网页正文读取失败，使用搜索摘要 / 使用搜索摘要） | 同 |
//! | 引擎显示名：`so_360` → 360 搜索、`cn_bing` → Bing 中国、`duckduckgo` → DuckDuckGo | [`provider_name`] |
//!
//! 数据来自工具调用的参数（关键词）与结果 JSON（[`Payload`]，字段同 engine `tools/websearch`）。

use super::state::{ToolStatus, ToolView};
use super::think_block::{loader, sheen_layer};
use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, fonts, tokens::metrics as m};
use gpui::{AnyElement, App, ClickEvent, FontWeight, Hsla, ScrollHandle, SharedString, Window, div, prelude::*, px};
use std::rc::Rc;

/// 走专用卡片的工具名（v1 `ToolSection` 的分派条件）
pub fn is_web_search(tool_name: &str) -> bool {
    tool_name == "websearch"
}

/// 单条搜索结果
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResultItem {
    /// 序号
    pub rank: Option<u32>,
    /// 来源引擎
    pub source: Option<String>,
    /// 标题
    pub title: Option<String>,
    /// 链接
    pub url: Option<String>,
    /// 摘要
    pub snippet: Option<String>,
    /// 网页正文
    pub content: Option<String>,
    /// 正文读取失败原因
    pub fetch_error: Option<String>,
}

/// 单个搜索源的状态
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderStatus {
    /// 引擎名
    pub name: Option<String>,
}

/// 结果 JSON
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Payload {
    /// `ok` / `partial` / `unavailable`
    pub status: Option<String>,
    /// 关键词
    pub query: Option<String>,
    /// 引擎（`a+b`）
    pub provider: Option<String>,
    /// 各引擎状态
    pub providers: Vec<ProviderStatus>,
    /// 说明
    pub note: Option<String>,
    /// 结果
    pub results: Vec<ResultItem>,
}

fn text(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

/// v1 `parseResult`：解析失败 / 不是对象按空处理；字段缺失或类型不对按缺省
pub fn parse_result(result: Option<&str>) -> Payload {
    let Some(value) = result.and_then(|r| serde_json::from_str::<serde_json::Value>(r).ok()).filter(serde_json::Value::is_object) else { return Payload::default() };
    let list = |key: &str| value.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default();
    Payload {
        status: text(&value, "status"),
        query: text(&value, "query"),
        provider: text(&value, "provider"),
        providers: list("providers").iter().map(|p| ProviderStatus { name: text(p, "name") }).collect(),
        note: text(&value, "note"),
        results: list("results")
            .iter()
            .map(|r| ResultItem {
                rank: r.get("rank").and_then(|v| v.as_u64()).map(|n| n as u32),
                source: text(r, "source"),
                title: text(r, "title"),
                url: text(r, "url"),
                snippet: text(r, "snippet"),
                content: text(r, "content"),
                fetch_error: text(r, "fetch_error"),
            })
            .collect(),
    }
}

/// v1 `parseQuery`：取参数里的 `query`
pub fn parse_query(arguments: &str) -> String {
    serde_json::from_str::<serde_json::Value>(arguments).ok().and_then(|v| v.get("query")?.as_str().map(|q| q.trim().to_string())).unwrap_or_default()
}

/// v1 `providerName`
pub fn provider_name(provider: &str) -> String {
    match provider.to_lowercase().as_str() {
        "so_360" => "360 搜索".into(),
        "cn_bing" => "Bing 中国".into(),
        "duckduckgo" => "DuckDuckGo".into(),
        _ => provider.to_string(),
    }
}

/// v1 `providerLabel`
pub fn provider_label(payload: &Payload) -> String {
    let named: Vec<String> = payload.providers.iter().filter_map(|p| p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(provider_name)).collect();
    if !named.is_empty() {
        return named.join(" + ");
    }
    if let Some(provider) = payload.provider.as_deref().filter(|p| !p.is_empty()) {
        return provider.split('+').filter(|p| !p.is_empty()).map(provider_name).collect::<Vec<_>>().join(" + ");
    }
    "360 搜索 + DuckDuckGo".into()
}

/// 状态：`(是否进行中, 是否不可用)`
pub fn state_of(status: ToolStatus, payload: &Payload) -> (bool, bool) {
    let active = matches!(status, ToolStatus::Calling | ToolStatus::Executing);
    (active, status == ToolStatus::Error || payload.status.as_deref() == Some("unavailable"))
}

/// 标题栏状态文案
pub fn label(status: ToolStatus, active: bool, unavailable: bool) -> &'static str {
    if active {
        "正在搜索网络"
    } else if unavailable {
        "网络搜索不可用，已继续回答"
    } else if status == ToolStatus::Interrupted {
        "网络搜索已中断"
    } else {
        "网络搜索完成"
    }
}

/// 结果项的正文读取状态：`(文案, 色调)`
pub fn fetch_state(item: &ResultItem) -> (&'static str, Fetch) {
    if item.content.as_deref().is_some_and(|c| !c.is_empty()) {
        ("已读取网页正文", Fetch::Fetched)
    } else if item.fetch_error.as_deref().is_some_and(|e| !e.is_empty()) {
        ("网页正文读取失败，使用搜索摘要", Fetch::Failed)
    } else {
        ("使用搜索摘要", Fetch::Summary)
    }
}

/// 正文读取状态的色调
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fetch {
    /// 已读取
    Fetched,
    /// 失败
    Failed,
    /// 仅摘要
    Summary,
}

/// 打开链接的回调
pub type OpenFn = Rc<dyn Fn(&str, &mut App)>;

fn mix(c: gpui::Rgba, alpha: f32) -> Hsla {
    Hsla::from(c).opacity(alpha)
}

/// 渲染卡片。`scroll` 为展开内容的卡内滚动句柄。
#[allow(clippy::too_many_arguments)]
pub fn block(
    id: SharedString,
    tool: &ToolView,
    expanded: bool,
    now_ms: f64,
    scroll: &ScrollHandle,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    open: OpenFn,
    window: &mut Window,
    cx: &App,
) -> AnyElement {
    let c = cx.buddy_theme().colors;
    let payload = parse_result(tool.result.as_deref());
    let (active, unavailable) = state_of(tool.status, &payload);
    let reduce = crate::accessibility::prefers_reduced_motion();
    if active && !reduce {
        window.request_animation_frame();
    }
    let query = Some(payload.query.as_deref().map(str::trim).unwrap_or_default().to_string()).filter(|q| !q.is_empty()).unwrap_or_else(|| parse_query(&tool.arguments));
    let icon_color = if unavailable { c.text_muted } else { c.tool_ui_accent };

    let header = div()
        .id(id.clone())
        .w_full()
        .min_h(px(28.0))
        .px(px(m::SPACE_2))
        .py(px(m::SPACE_1))
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .text_color(c.text_muted)
        .cursor_pointer()
        .hover(|s| s.bg(c.bg_sunken))
        .on_click(on_toggle)
        .child(div().flex_none().text_color(icon_color).child(icon(IconName::Search, px(14.0))))
        .child(div().flex_none().text_size(px(m::FONT_SIZE_SM)).font_weight(FontWeight(600.0)).child(label(tool.status, active, unavailable)))
        .children(active.then(|| loader(c.tool_ui_accent, now_ms, reduce)))
        .when(!query.is_empty(), |d| {
            d.child(
                div()
                    .min_w_0()
                    .flex()
                    .text_size(px(m::FONT_SIZE_XS))
                    .text_color(c.text_muted)
                    .child(div().flex_none().mr(px(m::SPACE_2)).child("·"))
                    .child(div().min_w_0().truncate().child(SharedString::from(query.clone()))),
            )
        })
        .child(div().flex_1())
        .child(div().flex_none().size(px(m::SPACE_4)).flex().items_center().justify_center().text_color(c.text_tertiary).child(icon(if expanded { IconName::ChevronDown } else { IconName::ChevronRight }, px(13.0))));

    let body = expanded.then(|| {
        let meta_row = |name: &'static str, value: String| {
            div()
                .min_w_0()
                .flex()
                .items_baseline()
                .gap(px(m::SPACE_2))
                .text_size(px(m::FONT_SIZE_XS))
                .child(div().flex_none().w(px(m::SPACE_12 + m::SPACE_2)).font_weight(FontWeight(600.0)).text_color(c.text_tertiary).child(name))
                .child(div().flex_1().min_w_0().font_weight(FontWeight(500.0)).text_color(c.text_primary).child(SharedString::from(value)))
        };
        let chip = |text: SharedString| {
            div()
                .flex_none()
                .min_h(px(m::SPACE_5))
                .px(px(m::SPACE_2))
                .py(px(1.0))
                .border_1()
                .border_color(c.border_subtle)
                .rounded(px(m::RADIUS_FULL))
                .flex()
                .items_center()
                .text_size(px(m::FONT_SIZE_XS))
                .text_color(c.text_tertiary)
                .bg(c.bg_elevated)
                .whitespace_nowrap()
                .child(text)
        };
        let mono = fonts::mono_font(cx).family;
        let empty = |text: &'static str| {
            div()
                .p(px(m::SPACE_3))
                .border_1()
                .border_dashed()
                .border_color(c.border_subtle)
                .rounded(px(m::RADIUS_MD))
                .flex()
                .justify_center()
                .text_size(px(m::FONT_SIZE_XS))
                .text_color(c.text_tertiary)
                .child(text)
                .into_any_element()
        };

        let results: AnyElement = if active {
            empty("正在等待搜索结果…")
        } else if !payload.results.is_empty() {
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().mb(px(m::SPACE_2)).text_size(px(m::FONT_SIZE_XS)).font_weight(FontWeight(600.0)).text_color(c.text_tertiary).child(SharedString::from(format!("搜索结果（{}）", payload.results.len()))))
                .child(div().flex().flex_col().gap(px(m::SPACE_2)).children(payload.results.iter().enumerate().map(|(index, item)| {
                    let rank = item.rank.unwrap_or(index as u32 + 1);
                    let title = item.title.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(str::to_string).or_else(|| item.url.clone()).unwrap_or_else(|| format!("结果 {rank}"));
                    let (fetch_text, fetch) = fetch_state(item);
                    let (fetch_fg, fetch_border): (Hsla, Hsla) = match fetch {
                        Fetch::Fetched => (c.state_success.into(), mix(c.state_success, 0.24)),
                        Fetch::Failed => (c.state_warning.into(), mix(c.state_warning, 0.24)),
                        Fetch::Summary => (c.text_tertiary.into(), c.border_subtle.into()),
                    };
                    let heading = div()
                        .min_w_0()
                        .flex()
                        .items_start()
                        .gap(px(m::SPACE_2))
                        .child(
                            div()
                                .flex_none()
                                .size(px(m::SPACE_5))
                                .rounded(px(m::RADIUS_FULL))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(c.tool_ui_accent_strong)
                                .bg(c.tool_ui_accent_soft)
                                .font_family(mono.clone())
                                .text_size(px(m::FONT_SIZE_XS))
                                .font_weight(FontWeight(600.0))
                                .line_height(px(m::FONT_SIZE_XS))
                                .child(SharedString::from(rank.to_string())),
                        )
                        .children(item.source.as_deref().filter(|s| !s.is_empty()).map(|s| chip(SharedString::from(provider_name(s)))))
                        .child(match item.url.clone() {
                            Some(url) => {
                                let open = open.clone();
                                div()
                                    .id(SharedString::from(format!("{id}-link-{index}")))
                                    .min_w_0()
                                    .flex()
                                    .items_center()
                                    .gap(px(m::SPACE_1))
                                    .text_size(px(m::FONT_SIZE_SM))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(c.tool_ui_accent_strong)
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(c.tool_ui_accent))
                                    .on_click(move |_, _, cx| open(&url, cx))
                                    .child(div().min_w_0().child(SharedString::from(title)))
                                    .child(div().flex_none().child(icon(IconName::ExternalLink, px(12.0))))
                                    .into_any_element()
                            }
                            None => div().min_w_0().text_size(px(m::FONT_SIZE_SM)).font_weight(FontWeight(600.0)).text_color(c.text_primary).child(SharedString::from(title)).into_any_element(),
                        });
                    div()
                        .min_w_0()
                        .p(px(m::SPACE_2))
                        .border_1()
                        .border_color(c.border_subtle)
                        .rounded(px(m::RADIUS_MD))
                        .bg(c.panel_surface)
                        .child(heading)
                        .children(item.snippet.as_deref().filter(|s| !s.is_empty()).map(|snippet| {
                            div()
                                .mt(px(m::SPACE_1))
                                .ml(px(m::SPACE_5 + m::SPACE_2))
                                .line_clamp(3)
                                .text_size(px(m::FONT_SIZE_XS))
                                .line_height(px(m::FONT_SIZE_XS * m::LINE_HEIGHT_BASE))
                                .text_color(c.text_muted)
                                .child(SharedString::from(snippet.to_string()))
                        }))
                        .child(
                            div().mt(px(m::SPACE_2)).ml(px(m::SPACE_5 + m::SPACE_2)).flex().child(
                                div()
                                    .min_h(px(m::SPACE_5))
                                    .px(px(m::SPACE_2))
                                    .py(px(1.0))
                                    .border_1()
                                    .border_color(fetch_border)
                                    .rounded(px(m::RADIUS_FULL))
                                    .flex()
                                    .items_center()
                                    .text_size(px(m::FONT_SIZE_XS))
                                    .line_height(px(m::FONT_SIZE_XS))
                                    .text_color(fetch_fg)
                                    .bg(c.bg_elevated)
                                    .child(fetch_text),
                            ),
                        )
                })))
                .into_any_element()
        } else {
            empty(if unavailable { "没有可展示的搜索结果" } else { "搜索未返回结果" })
        };

        div()
            .id(SharedString::from(format!("{id}-body")))
            .w_full()
            .max_h(px(m::SPACE_12 * 7.0))
            .overflow_y_scroll()
            .track_scroll(scroll)
            .p(px(m::SPACE_3))
            .border_t_1()
            .border_color(c.border_subtle)
            .flex()
            .flex_col()
            .gap(px(m::SPACE_3))
            .bg(mix(c.bg_sunken, 0.62))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(m::SPACE_2))
                    .child(meta_row("搜索内容", if query.is_empty() { "未获得搜索关键词".to_string() } else { query.clone() }))
                    .child(meta_row("搜索引擎", provider_label(&payload))),
            )
            .children(payload.note.as_deref().filter(|n| !n.is_empty()).map(|note| {
                div().text_size(px(m::FONT_SIZE_XS)).line_height(px(m::FONT_SIZE_XS * m::LINE_HEIGHT_BASE)).text_color(c.text_tertiary).child(SharedString::from(note.to_string()))
            }))
            .child(results)
    });

    div()
        .relative()
        .w_full()
        .my(px(m::SPACE_2))
        .overflow_hidden()
        .border_1()
        .border_color(c.border_default)
        .rounded_tr(px(m::RADIUS_MD))
        .rounded_br(px(m::RADIUS_MD))
        .bg(c.panel_surface)
        .children((active && !reduce).then(|| sheen_layer(c.tool_ui_flow_highlight, now_ms)))
        .child(div().absolute().left_0().top_0().bottom_0().w(px(2.0)).bg(c.tool_ui_accent))
        .child(div().relative().child(header).children(body))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_names_follow_v1() {
        assert_eq!(provider_name("SO_360"), "360 搜索");
        assert_eq!(provider_name("cn_bing"), "Bing 中国");
        assert_eq!(provider_name("duckduckgo"), "DuckDuckGo");
        assert_eq!(provider_name("other"), "other");
    }

    #[test]
    fn provider_label_prefers_status_list_then_joined_string_then_default() {
        let with_list = parse_result(Some(r#"{"providers":[{"name":"so_360"},{"name":" "},{"name":"duckduckgo"}],"provider":"x"}"#));
        assert_eq!(provider_label(&with_list), "360 搜索 + DuckDuckGo");
        let joined = parse_result(Some(r#"{"provider":"so_360+duckduckgo"}"#));
        assert_eq!(provider_label(&joined), "360 搜索 + DuckDuckGo");
        assert_eq!(provider_label(&Payload::default()), "360 搜索 + DuckDuckGo", "两者都没有：v1 的默认文案");
    }

    #[test]
    fn labels_and_states_follow_v1() {
        let unavailable = parse_result(Some(r#"{"status":"unavailable"}"#));
        assert_eq!(state_of(ToolStatus::Executing, &Payload::default()), (true, false));
        assert_eq!(state_of(ToolStatus::Done, &unavailable), (false, true));
        assert_eq!(state_of(ToolStatus::Error, &Payload::default()), (false, true));
        assert_eq!(label(ToolStatus::Executing, true, false), "正在搜索网络");
        assert_eq!(label(ToolStatus::Done, false, true), "网络搜索不可用，已继续回答");
        assert_eq!(label(ToolStatus::Interrupted, false, false), "网络搜索已中断");
        assert_eq!(label(ToolStatus::Done, false, false), "网络搜索完成");
    }

    #[test]
    fn query_comes_from_the_result_then_the_arguments() {
        assert_eq!(parse_query(r#"{"query":"  buddy  "}"#), "buddy");
        assert_eq!(parse_query("{半截"), "");
        assert_eq!(parse_result(Some("不是 JSON")), Payload::default());
        assert_eq!(parse_result(None), Payload::default());
    }

    #[test]
    fn fetch_state_labels() {
        let fetched = ResultItem { content: Some("正文".into()), fetch_error: Some("x".into()), ..Default::default() };
        assert_eq!(fetch_state(&fetched), ("已读取网页正文", Fetch::Fetched), "有正文优先于错误");
        let failed = ResultItem { fetch_error: Some("超时".into()), ..Default::default() };
        assert_eq!(fetch_state(&failed), ("网页正文读取失败，使用搜索摘要", Fetch::Failed));
        assert_eq!(fetch_state(&ResultItem::default()), ("使用搜索摘要", Fetch::Summary));
    }

    #[test]
    fn engine_payload_shape_is_understood() {
        let payload = parse_result(Some(
            r#"{"status":"partial","query":"buddy","provider":"so_360+duckduckgo","note":"部分","results":[
                {"rank":1,"source":"so_360","title":"标题","url":"https://a.example","snippet":"摘要","content":"正文"},
                {"source":"duckduckgo","url":"https://b.example","fetch_error":"超时"}]}"#,
        ));
        assert_eq!(payload.results.len(), 2);
        assert_eq!(payload.results[0].rank, Some(1));
        assert_eq!(payload.results[1].rank, None, "缺序号时按位置补");
        assert_eq!(payload.note.as_deref(), Some("部分"));
    }
}
