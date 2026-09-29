//! ask_user 提问卡（S05-13）—— 对应 v1 `AskUserCard.tsx`、`QuestionPrompt.tsx`、`utils/askUserDisplay.ts` 与 `.tool-question-*` / `.question-prompt*` 样式
//!
//! | v1 | 本模块 |
//! |----|------|
//! | 问题块：图标 + 标题（「模型正在等待你的回答」等）+ 短标签 + 「多选」标记 + 问题文本 | [`AskUserCard`]、[`prompt_title`] |
//! | 选项：等待回答时可点（单选替换 / 多选切换，选中带对勾）；否则只读展示；「需补充」标记与说明 | 同 |
//! | 选中且要求补充的选项，下面出现「{标签} · 补充信息」输入框 | 每个选项一个单行 [`TextArea`] |
//! | 「或输入自定义回答」多行框，Cmd / Ctrl+Enter 提交（Enter 换行） | [`EnterMode::NewlineOnEnter`] |
//! | 确认 / 跳过；提交中禁用；失败提示「提交失败，请重试」；可提交条件 | [`can_submit`]、[`build_answer`] |
//! | 已有结果时显示「用户回应」 | 同 |
//! | 参数解析：只取问题最后一句、兼容 `multi_select` / `multiSelect` 等两种写法 | [`parse_arguments`]、[`extract_question`] |
//!
//! 回答经 [`AnswerFn`] 交给路由器 → `ChatEngine::answer_tool_question`（按调用 id 配对）。

use super::state::Question;
use crate::icons::{IconName, icon};
use crate::text_area::{EnterMode, TextArea, TextAreaEvent, TextAreaStyle};
use crate::theme_system::{BuddyTheme, box_shadows, tokens::metrics as m};
use gpui::{
    AnyElement, App, Context, Entity, Focusable, FontWeight, Hsla, SharedString, Subscription, Window, div, linear_color_stop, linear_gradient, prelude::*, px,
};
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

// ───────────────────────── 参数解析（v1 askUserDisplay.ts）─────────────────────────

/// 选项
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OptionView {
    /// 标签
    pub label: String,
    /// 说明
    pub description: String,
    /// 是否要求补充输入
    pub requires_input: bool,
    /// 补充输入的占位符
    pub input_placeholder: String,
}

/// 用于显示的问题
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AskDisplay {
    /// 短标签
    pub header: Option<String>,
    /// 问题
    pub question: String,
    /// 选项
    pub options: Vec<OptionView>,
    /// 多选
    pub multi_select: bool,
}

fn strip_prefix_ws<'a>(line: &'a str, is_marker: impl Fn(char) -> bool, max: usize) -> &'a str {
    // 前缀 = 1..=max 个标记字符 + 至少一个空白
    let marks = line.chars().take_while(|c| is_marker(*c)).count();
    if marks == 0 || marks > max {
        return line;
    }
    let rest = &line[line.char_indices().nth(marks).map_or(line.len(), |(i, _)| i)..];
    let trimmed = rest.trim_start();
    if trimmed.len() < rest.len() { trimmed } else { line }
}

fn clean_line(line: &str) -> String {
    // 与 v1 相同的顺序：标题 → 有序 → 无序 → 引用，再去掉反引号与粗体标记
    let line = strip_prefix_ws(line, |c| c == '#', 6);
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    let line = if digits > 0 {
        let after = &line[digits..];
        match after.chars().next() {
            Some('.' | ')' | '、') => {
                let rest = &after[after.chars().next().unwrap().len_utf8()..];
                let trimmed = rest.trim_start();
                if trimmed.len() < rest.len() { trimmed } else { line }
            }
            _ => line,
        }
    } else {
        line
    };
    let line = match line.chars().next() {
        Some('-' | '*' | '+') => {
            let rest = &line[1..];
            let trimmed = rest.trim_start();
            if trimmed.len() < rest.len() { trimmed } else { line }
        }
        _ => line,
    };
    let line = line.trim_start_matches('>').trim_start_matches(|c: char| c.is_whitespace() && line.starts_with('>'));
    line.replace('`', "").replace("**", "").trim().to_string()
}

/// v1 `extractAskUserQuestion`：取以问号结尾的最后一行，否则取最后一个非空行
pub fn extract_question(raw: &str) -> String {
    let cleaned = raw.trim();
    if cleaned.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = cleaned.lines().map(clean_line).filter(|l| !l.is_empty()).collect();
    if let Some(line) = lines.iter().rev().find(|l| l.ends_with('?') || l.ends_with('？')) {
        return line.clone();
    }
    lines.last().cloned().unwrap_or_else(|| clean_line(cleaned))
}

fn normalize_option(option: &serde_json::Value) -> Option<OptionView> {
    let record = option.as_object()?;
    let text = |key: &str| record.get(key).and_then(|v| v.as_str());
    let label = text("label")?.trim().to_string();
    if label.is_empty() {
        return None;
    }
    let flag = record.get("requiresInput").and_then(|v| v.as_bool()).unwrap_or_else(|| record.get("requires_input").and_then(|v| v.as_bool()) == Some(true));
    Some(OptionView {
        label,
        description: text("description").map(|d| d.trim().to_string()).unwrap_or_default(),
        requires_input: flag,
        input_placeholder: text("inputPlaceholder").or_else(|| text("input_placeholder")).unwrap_or_default().to_string(),
    })
}

/// v1 `parseAskUserArguments`（参数不是 JSON 对象时，把整段当问题）
pub fn parse_arguments(raw: &str) -> AskDisplay {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(serde_json::Value::Object(record)) => {
            let raw_question = record.get("question").and_then(|q| q.as_str()).unwrap_or(raw);
            let options = record.get("options").and_then(|o| o.as_array()).map(|o| o.iter().filter_map(normalize_option).collect()).unwrap_or_default();
            let flag = |key: &str| record.get(key).and_then(|v| v.as_bool()) == Some(true);
            AskDisplay {
                header: record.get("header").and_then(|h| h.as_str()).map(|h| h.trim().to_string()),
                question: extract_question(raw_question),
                options,
                multi_select: flag("multi_select") || flag("multiSelect"),
            }
        }
        _ => AskDisplay { question: extract_question(raw), ..Default::default() },
    }
}

/// 引擎推来的待答问题 → 显示数据
pub fn from_question(q: &Question) -> AskDisplay {
    AskDisplay {
        header: Some(q.header.clone()).filter(|h| !h.is_empty()),
        question: q.question.clone(),
        options: q
            .options
            .iter()
            .map(|o| OptionView { label: o.label.clone(), description: o.description.clone(), requires_input: o.requires_input, input_placeholder: o.input_placeholder.clone() })
            .collect(),
        multi_select: q.multi_select,
    }
}

// ───────────────────────── 交互逻辑（v1 AskUserCard）─────────────────────────

/// 提示标题
pub fn prompt_title(interrupted: bool, has_result: bool, awaiting: bool, submitted: bool) -> &'static str {
    if interrupted {
        "询问已中断"
    } else if has_result {
        "询问已完成"
    } else if awaiting {
        "模型正在等待你的回答"
    } else if submitted {
        "回答已提交"
    } else {
        "模型正在准备问题"
    }
}

/// v1 `toggleOption`：多选切换，单选替换
pub fn toggle(selected: &BTreeSet<usize>, index: usize, multi: bool) -> BTreeSet<usize> {
    let mut next = selected.clone();
    if multi {
        if !next.remove(&index) {
            next.insert(index);
        }
    } else {
        next.clear();
        next.insert(index);
    }
    next
}

/// v1 `canSubmit`：有自定义回答，或选了选项且所有要求补充的选项都已填
pub fn can_submit(submitting: bool, options: &[OptionView], selected: &BTreeSet<usize>, inputs: &HashMap<usize, String>, custom: &str) -> bool {
    let missing = selected.iter().any(|i| options.get(*i).is_some_and(|o| o.requires_input) && inputs.get(i).is_none_or(|t| t.trim().is_empty()));
    !submitting && (!custom.trim().is_empty() || (!selected.is_empty() && !missing))
}

/// 回答
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    /// 选中的选项下标（升序）
    pub selected: Vec<usize>,
    /// 与 `selected` 逐项对应的补充输入
    pub inputs: Vec<String>,
    /// 自定义回答
    pub custom: Option<String>,
}

/// v1 `handleSubmit`：自定义回答与已选选项（含补充输入）一起提交，避免静默丢弃用户的选择
pub fn build_answer(selected: &BTreeSet<usize>, inputs: &HashMap<usize, String>, custom: &str) -> Answer {
    let indexes: Vec<usize> = selected.iter().copied().collect();
    let inputs = indexes.iter().map(|i| inputs.get(i).map(|t| t.trim().to_string()).unwrap_or_default()).collect();
    let custom = Some(custom.trim().to_string()).filter(|c| !c.is_empty());
    Answer { selected: indexes, inputs, custom }
}

/// 底部提示（v1）
pub fn hint_text(multi_select: bool) -> &'static str {
    if multi_select { "可选择多个选项" } else { "请选择一个选项，或填写自定义回答" }
}

/// 回答回调：`(调用 id, 回答)` → 交给 engine；`Err` 表示被拒绝（如已超时）
pub type AnswerFn = Rc<dyn Fn(&str, Answer, &mut App) -> Result<(), String>>;

/// 卡片的外部输入（每次渲染由列表同步）
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CardInput {
    /// 显示数据
    pub display: AskDisplay,
    /// 是否正在等待这条回答
    pub awaiting: bool,
    /// 工具已有结果
    pub has_result: bool,
    /// 已中断
    pub interrupted: bool,
    /// 用户回应
    pub result: Option<String>,
}

// ───────────────────────── 卡片实体 ─────────────────────────

/// 提问卡
pub struct AskUserCard {
    id: String,
    input: CardInput,
    selected: BTreeSet<usize>,
    option_inputs: HashMap<usize, Entity<TextArea>>,
    custom: Entity<TextArea>,
    submitting: bool,
    submitted: bool,
    error: Option<String>,
    on_answer: AnswerFn,
    _subscriptions: Vec<Subscription>,
}

fn field_style(theme: &crate::theme_system::Theme, min_lines: f32, max_lines: f32) -> TextAreaStyle {
    let c = theme.colors;
    let line = m::FONT_SIZE_SM * m::LINE_HEIGHT_BASE;
    TextAreaStyle {
        font_size: px(m::FONT_SIZE_SM),
        line_height: px(line),
        max_height: Some(px(line * max_lines)),
        min_height: px(line * min_lines),
        text_color: c.text_primary.into(),
        placeholder_color: c.text_tertiary.into(),
        caret_color: c.text_primary.into(),
        selection_color: Hsla::from(c.buddy_primary).opacity(crate::markdown::SELECTION_ALPHA),
    }
}

impl AskUserCard {
    /// 新建
    pub fn new(id: String, on_answer: AnswerFn, cx: &mut Context<Self>) -> Self {
        let theme = *cx.buddy_theme();
        let custom = cx.new(|cx| {
            let mut area = TextArea::new("输入你的回答…", field_style(&theme, 2.0, 6.0), cx);
            area.set_enter_mode(EnterMode::NewlineOnEnter);
            area
        });
        let subscriptions = vec![cx.subscribe(&custom, |this: &mut Self, _, event: &TextAreaEvent, cx| match event {
            TextAreaEvent::Submit => this.submit(cx),
            TextAreaEvent::Changed => {
                this.error = None;
                cx.notify();
            }
        })];
        Self {
            id,
            input: CardInput::default(),
            selected: BTreeSet::new(),
            option_inputs: HashMap::new(),
            custom,
            submitting: false,
            submitted: false,
            error: None,
            on_answer,
            _subscriptions: subscriptions,
        }
    }

    /// 同步外部数据。变为「等待回答」时重置本地状态（v1 `useEffect`）；有变化才通知
    pub fn sync(&mut self, input: CardInput, cx: &mut Context<Self>) {
        if self.input == input {
            return;
        }
        if input.awaiting && !self.input.awaiting {
            self.selected.clear();
            self.option_inputs.clear();
            self.submitting = false;
            self.submitted = false;
            self.error = None;
            self.custom.update(cx, |c, cx| c.set_text("", cx));
        }
        self.input = input;
        cx.notify();
    }

    /// 选中的选项（自检用）
    pub fn selected(&self) -> Vec<usize> {
        self.selected.iter().copied().collect()
    }

    /// 是否已提交（自检用）
    pub fn submitted(&self) -> bool {
        self.submitted
    }

    /// 自定义回答框（自检用）
    pub fn custom_area(&self) -> &Entity<TextArea> {
        &self.custom
    }

    /// 补充信息框（自检用）
    pub fn option_input(&self, index: usize) -> Option<&Entity<TextArea>> {
        self.option_inputs.get(&index)
    }

    /// 点选选项
    pub fn toggle_option(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.input.awaiting || self.submitting {
            return;
        }
        self.selected = toggle(&self.selected, index, self.input.display.multi_select);
        let theme = *cx.buddy_theme();
        for &i in &self.selected {
            let Some(option) = self.input.display.options.get(i) else { continue };
            if option.requires_input && !self.option_inputs.contains_key(&i) {
                let placeholder = Some(option.input_placeholder.clone()).filter(|p| !p.is_empty()).unwrap_or_else(|| "请输入补充内容".into());
                let area = cx.new(|cx| {
                    let mut area = TextArea::new(placeholder, field_style(&theme, 1.0, 1.0), cx);
                    area.set_enter_mode(EnterMode::SingleLine);
                    area
                });
                self._subscriptions.push(cx.subscribe(&area, |this: &mut Self, _, event: &TextAreaEvent, cx| {
                    if *event == TextAreaEvent::Changed {
                        this.error = None;
                        cx.notify();
                    }
                }));
                self.option_inputs.insert(i, area);
            }
        }
        self.error = None;
        cx.notify();
    }

    fn input_texts(&self, cx: &App) -> HashMap<usize, String> {
        self.option_inputs.iter().map(|(i, a)| (*i, a.read(cx).text().to_string())).collect()
    }

    fn can_submit(&self, cx: &App) -> bool {
        can_submit(self.submitting, &self.input.display.options, &self.selected, &self.input_texts(cx), self.custom.read(cx).text())
    }

    fn deliver(&mut self, answer: Answer, cx: &mut Context<Self>) {
        self.submitting = true;
        self.error = None;
        let result = (self.on_answer)(&self.id, answer, cx);
        self.submitting = false;
        match result {
            Ok(()) => self.submitted = true,
            Err(_) => self.error = Some("提交失败，请重试".into()),
        }
        cx.notify();
    }

    /// 确认（Cmd / Ctrl+Enter 或点「确认」）
    pub fn submit(&mut self, cx: &mut Context<Self>) {
        if !self.input.awaiting || !self.can_submit(cx) {
            return;
        }
        let answer = build_answer(&self.selected, &self.input_texts(cx), self.custom.read(cx).text());
        self.deliver(answer, cx);
    }

    /// 跳过：空回答
    pub fn skip(&mut self, cx: &mut Context<Self>) {
        if !self.input.awaiting || self.submitting {
            return;
        }
        self.deliver(Answer { selected: Vec::new(), inputs: Vec::new(), custom: None }, cx);
    }
}

impl Render for AskUserCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.buddy_theme();
        let c = theme.colors;
        let display = &self.input.display;
        let awaiting = self.input.awaiting;
        let title = prompt_title(self.input.interrupted, self.input.has_result, awaiting, self.submitted);
        let can_submit = self.can_submit(cx);

        let chip = |text: &str, accent: bool| {
            div()
                .min_h(px(m::SPACE_5))
                .px(px(m::SPACE_2))
                .py(px(1.0))
                .border_1()
                .border_color(c.border_subtle)
                .rounded(px(m::RADIUS_FULL))
                .flex()
                .items_center()
                .max_w_full()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(m::FONT_SIZE_XS))
                .font_weight(FontWeight(500.0))
                .line_height(px(m::FONT_SIZE_XS))
                .text_color(if accent { c.tool_ui_accent_strong } else { c.text_tertiary })
                .bg(if accent { c.tool_ui_accent_soft } else { c.bg_elevated })
                .child(SharedString::from(text.to_string()))
        };

        let prompt = div()
            .w_full()
            .px(px(m::SPACE_3))
            .py(px(m::SPACE_2))
            .border_1()
            .border_color(c.tool_ui_accent_medium)
            .rounded(px(m::RADIUS_MD))
            .flex()
            .items_start()
            .gap(px(m::SPACE_2))
            .text_color(c.text_primary)
            .bg(c.composer_surface)
            .shadow(box_shadows(theme.shadows.shadow_static))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .rounded(px(m::RADIUS_MD))
                    .bg(linear_gradient(135., linear_color_stop(c.tool_ui_accent_soft, 0.), linear_color_stop(Hsla::from(c.tool_ui_accent_soft).opacity(0.), 0.64))),
            )
            .relative()
            .child(
                div()
                    .flex_none()
                    .size(px(m::SPACE_6))
                    .border_1()
                    .border_color(c.tool_ui_accent_medium)
                    .rounded(px(m::RADIUS_MD))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(c.tool_ui_accent_strong)
                    .bg(c.tool_ui_accent_soft)
                    .child(icon(IconName::CircleHelp, px(14.0))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .min_h(px(m::SPACE_5))
                            .mb(px(m::SPACE_1))
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(m::SPACE_1))
                            .child(div().text_size(px(m::FONT_SIZE_XS)).font_weight(FontWeight(700.0)).text_color(c.tool_ui_accent_strong).child(title))
                            .children(display.header.as_deref().filter(|h| !h.is_empty()).map(|h| chip(h, false)))
                            .when(display.multi_select, |d| d.child(chip("多选", true))),
                    )
                    .when(!display.question.is_empty(), |d| {
                        d.child(div().text_size(px(m::FONT_SIZE_BASE)).font_weight(FontWeight(500.0)).line_height(px(m::FONT_SIZE_BASE * m::LINE_HEIGHT_BASE)).text_color(c.text_primary).child(SharedString::from(display.question.clone())))
                    }),
            );

        let options = (!display.options.is_empty()).then(|| {
            div().flex().flex_wrap().gap(px(m::SPACE_2)).children(display.options.iter().enumerate().map(|(index, option)| {
                let selected = self.selected.contains(&index);
                let body = |d: gpui::Stateful<gpui::Div>| {
                    d.when(selected, |d| d.child(icon(IconName::Check, px(12.0))))
                        .child(SharedString::from(option.label.clone()))
                        .when(option.requires_input, |d| {
                            d.child(
                                div()
                                    .px(px(m::SPACE_1))
                                    .rounded(px(m::RADIUS_FULL))
                                    .text_size(px(9.0))
                                    .font_weight(FontWeight(600.0))
                                    .text_color(c.tool_ui_accent_strong)
                                    .bg(c.tool_ui_accent_soft)
                                    .child("需补充"),
                            )
                        })
                        .when(!option.description.is_empty(), |d| {
                            d.child(
                                div()
                                    .max_w(px(220.0))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_color(c.text_muted)
                                    .child(SharedString::from(format!("— {}", option.description))),
                            )
                        })
                };
                let base = div()
                    .id(SharedString::from(format!("ask-option-{index}")))
                    .max_w_full()
                    .px(px(m::SPACE_2))
                    .py(px(m::SPACE_1))
                    .border_1()
                    .rounded(px(m::RADIUS_MD))
                    .flex()
                    .items_center()
                    .gap(px(m::SPACE_1))
                    .text_size(px(m::FONT_SIZE_XS))
                    .border_color(if selected { c.tool_ui_accent } else { c.border_subtle })
                    .text_color(if selected { c.tool_ui_accent_strong } else { c.text_primary })
                    .bg(if selected { c.tool_ui_accent_soft } else { c.bg_elevated });
                if awaiting {
                    body(base)
                        .cursor_pointer()
                        .when(self.submitting, |d| d.opacity(0.6))
                        .when(!selected, |d| d.hover(|s| s.border_color(c.tool_ui_accent_medium).bg(c.tool_ui_accent_soft)))
                        .on_click(cx.listener(move |this, _, _, cx| this.toggle_option(index, cx)))
                        .into_any_element()
                } else {
                    body(base).into_any_element()
                }
            }))
        });

        let field = |area: &Entity<TextArea>, window: &Window, cx: &App, min_h: f32, pad_y: f32| {
            let focused = area.read(cx).focus_handle(cx).is_focused(window);
            div()
                .w_full()
                .min_h(px(min_h))
                .px(px(m::SPACE_2))
                .py(px(pad_y))
                .border_1()
                .border_color(if focused { c.tool_ui_accent } else { c.border_subtle })
                .rounded(px(m::RADIUS_MD))
                .bg(c.bg_surface)
                .child(area.clone())
        };
        let label_style = |d: gpui::Div| d.flex().items_center().gap(px(m::SPACE_1)).text_size(px(m::FONT_SIZE_XS)).font_weight(FontWeight(500.0)).text_color(c.text_muted);

        let followups: Vec<AnyElement> = if awaiting {
            self.selected
                .iter()
                .filter_map(|&i| {
                    let option = display.options.get(i).filter(|o| o.requires_input)?;
                    let area = self.option_inputs.get(&i)?;
                    Some(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(m::SPACE_1))
                            .child(label_style(div()).child(SharedString::from(format!("{} · 补充信息", option.label))))
                            .child(field(area, window, cx, m::SPACE_8, m::SPACE_1))
                            .into_any_element(),
                    )
                })
                .collect()
        } else {
            Vec::new()
        };

        let footer = awaiting.then(|| {
            let hint_error = self.error.clone();
            div()
                .flex()
                .flex_col()
                .gap(px(m::SPACE_3))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(m::SPACE_1))
                        .child(label_style(div()).child(icon(IconName::CornerDownRight, px(12.0))).child("或输入自定义回答"))
                        .child(field(&self.custom, window, cx, 3.0 * m::SPACE_4 + 1.0, m::SPACE_2)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(m::SPACE_2))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(m::FONT_SIZE_XS))
                                .text_color(if hint_error.is_some() { c.state_error } else { c.text_tertiary })
                                .child(SharedString::from(hint_error.unwrap_or_else(|| hint_text(display.multi_select).to_string()))),
                        )
                        .child(
                            div()
                                .id("ask-skip")
                                .flex_none()
                                .min_h(px(m::SPACE_8))
                                .px(px(m::SPACE_3))
                                .py(px(m::SPACE_1))
                                .border_1()
                                .border_color(c.border_default)
                                .rounded(px(m::RADIUS_MD))
                                .flex()
                                .items_center()
                                .text_size(px(m::FONT_SIZE_SM))
                                .line_height(px(m::FONT_SIZE_SM))
                                .text_color(c.text_muted)
                                .when(self.submitting, |d| d.opacity(0.45))
                                .when(!self.submitting, |d| d.cursor_pointer())
                                .on_click(cx.listener(|this, _, _, cx| this.skip(cx)))
                                .child("跳过"),
                        )
                        .child(
                            div()
                                .id("ask-submit")
                                .flex_none()
                                .min_h(px(m::SPACE_8))
                                .px(px(m::SPACE_3))
                                .py(px(m::SPACE_1))
                                .border_1()
                                .border_color(c.tool_ui_action)
                                .rounded(px(m::RADIUS_MD))
                                .flex()
                                .items_center()
                                .text_size(px(m::FONT_SIZE_SM))
                                .line_height(px(m::FONT_SIZE_SM))
                                .text_color(c.text_on_primary)
                                .bg(c.tool_ui_action)
                                .when(!can_submit, |d| d.opacity(0.45))
                                .when(can_submit, |d| d.cursor_pointer())
                                .on_click(cx.listener(|this, _, _, cx| this.submit(cx)))
                                .child(if self.submitting { "提交中" } else { "确认" }),
                        ),
                )
        });

        let answer = self.input.has_result.then(|| self.input.result.clone()).flatten().map(|result| {
            div()
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_2))
                .border_l_2()
                .border_color(c.tool_ui_accent)
                .rounded_r(px(m::RADIUS_MD))
                .flex()
                .gap(px(m::SPACE_2))
                .text_size(px(m::FONT_SIZE_SM))
                .text_color(c.text_muted)
                .bg(c.bg_elevated)
                .child("用户回应")
                .child(div().font_weight(FontWeight(500.0)).text_color(c.text_primary).child(SharedString::from(result)))
        });

        div()
            .flex()
            .flex_col()
            .gap(px(m::SPACE_3))
            .child(prompt)
            .children(options)
            .children(followups)
            .children(footer)
            .children(answer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_the_final_question_from_long_markdown() {
        let raw = "## 通过 Homebrew Cask 发布 macOS App\n\n```ruby\ncask \"buddy\" do\nend\n```\n\n你打算用哪种方式？或者需要我帮你生成一个完整的 Cask 文件示例？";
        assert_eq!(extract_question(raw), "你打算用哪种方式？或者需要我帮你生成一个完整的 Cask 文件示例？");
    }

    #[test]
    fn extraction_falls_back_to_the_last_line_and_cleans_markup() {
        assert_eq!(extract_question("  "), "");
        assert_eq!(extract_question("先说明\n- **重要** 选一个吧"), "重要 选一个吧");
        assert_eq!(extract_question("1. 第一步\n2) `第二步`"), "第二步");
        assert_eq!(extract_question("> 引用一下？\n然后陈述"), "引用一下？", "以问号结尾的最后一行优先");
        assert_eq!(extract_question("### 标题"), "标题");
    }

    #[test]
    fn normalizes_snake_case_ask_user_options() {
        let display = parse_arguments(
            r#"{"header":"发布方式","question":"请选择发布方式？","multi_select":true,
                "options":[{"label":"生成 Cask","description":"生成完整示例","requires_input":true,"input_placeholder":"输入包名"},
                           {"label":"  "},{"description":"没有标签"},"不是对象"]}"#,
        );
        assert_eq!(display.header.as_deref(), Some("发布方式"));
        assert_eq!(display.question, "请选择发布方式？");
        assert!(display.multi_select);
        assert_eq!(display.options, vec![OptionView { label: "生成 Cask".into(), description: "生成完整示例".into(), requires_input: true, input_placeholder: "输入包名".into() }]);
    }

    #[test]
    fn camel_case_and_non_json_arguments_are_understood() {
        let display = parse_arguments(r#"{"question":"选哪个？","multiSelect":true,"options":[{"label":"甲","requiresInput":true,"inputPlaceholder":"写点什么"}]}"#);
        assert!(display.multi_select && display.options[0].requires_input && display.options[0].input_placeholder == "写点什么");
        let plain = parse_arguments("直接一句话？");
        assert_eq!((plain.question.as_str(), plain.options.len(), plain.multi_select), ("直接一句话？", 0, false));
    }

    #[test]
    fn titles_follow_v1_priority() {
        assert_eq!(prompt_title(true, true, true, true), "询问已中断");
        assert_eq!(prompt_title(false, true, true, true), "询问已完成");
        assert_eq!(prompt_title(false, false, true, true), "模型正在等待你的回答");
        assert_eq!(prompt_title(false, false, false, true), "回答已提交");
        assert_eq!(prompt_title(false, false, false, false), "模型正在准备问题");
    }

    #[test]
    fn single_select_replaces_and_multi_select_toggles() {
        let one: BTreeSet<usize> = [0].into();
        assert_eq!(toggle(&one, 1, false), [1].into());
        assert_eq!(toggle(&one, 1, true), [0, 1].into());
        assert_eq!(toggle(&[0, 1].into(), 0, true), [1].into());
    }

    fn opts() -> Vec<OptionView> {
        vec![
            OptionView { label: "甲".into(), ..Default::default() },
            OptionView { label: "乙".into(), requires_input: true, ..Default::default() },
        ]
    }

    #[test]
    fn submit_needs_a_choice_or_custom_text_and_required_inputs() {
        let none = HashMap::new();
        let o = opts();
        assert!(!can_submit(false, &o, &BTreeSet::new(), &none, ""), "什么都没选");
        assert!(can_submit(false, &o, &[0].into(), &none, ""));
        assert!(!can_submit(false, &o, &[1].into(), &none, ""), "要求补充却没填");
        assert!(!can_submit(false, &o, &[1].into(), &HashMap::from([(1, "  ".to_string())]), ""), "只有空白也算没填");
        assert!(can_submit(false, &o, &[1].into(), &HashMap::from([(1, "内容".to_string())]), ""));
        assert!(can_submit(false, &o, &BTreeSet::new(), &none, "自定义"), "自定义回答单独即可");
        assert!(can_submit(false, &o, &[1].into(), &none, "自定义"), "有自定义回答时不检查补充");
        assert!(!can_submit(true, &o, &[0].into(), &none, ""), "提交中禁用");
    }

    #[test]
    fn answer_keeps_selection_inputs_and_custom_together() {
        let inputs = HashMap::from([(1, "  内容 ".to_string())]);
        let a = build_answer(&[1, 0].into(), &inputs, "  补充说明 ");
        assert_eq!(a, Answer { selected: vec![0, 1], inputs: vec!["".into(), "内容".into()], custom: Some("补充说明".into()) });
        assert_eq!(build_answer(&BTreeSet::new(), &HashMap::new(), "  ").custom, None);
    }
}
