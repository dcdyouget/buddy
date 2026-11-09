//! 历史搜索覆盖层：独立输入、按提交查询、保留阅读状态并在一小时后静默过期。

mod shortcut;
mod state;

pub(super) use shortcut::{LABEL as SHORTCUT_LABEL, matches_keystroke};

use super::{
    composer::{Composer, ComposerEvent},
    drag,
    message_actions::format_time,
};
use crate::{
    chat_bridge::spawn_engine,
    components::{IconButtonVariant, TextTooltip, icon_button},
    icons::IconName,
    theme_system::{BuddyTheme, TextScale, tokens::metrics as m},
};
use buddy_engine::{chat::ChatEngine, storage::HistoryMatch};
use gpui::{
    Context, Entity, EventEmitter, FocusHandle, Focusable, ListAlignment, ListState, Render,
    SharedString, Subscription, Task, Window, div, list, prelude::*, px,
};
use state::{RESULT_LIFETIME, SearchState};
use std::{collections::HashSet, sync::Arc, time::SystemTime};

pub(super) enum SearchEvent {
    Back,
    Locate(HistoryMatch),
}

pub(super) struct HistorySearch {
    engine: Arc<ChatEngine>,
    composer: Entity<Composer>,
    state: SearchState,
    list: ListState,
    expanded: HashSet<String>,
    selected: Option<String>,
    active: bool,
    back: FocusHandle,
    results_focus: FocusHandle,
    drag_source: drag::DragSource,
    query_task: Option<Task<()>>,
    expiry_task: Option<Task<()>>,
    locating: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SearchEvent> for HistorySearch {}

impl HistorySearch {
    pub fn new(
        engine: Arc<ChatEngine>,
        source: drag::DragSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let composer = cx.new(|cx| Composer::for_history_search(window, cx));
        composer.update(cx, |composer, cx| {
            composer.set_drag_source(source.clone());
            composer.set_enabled(false, cx);
        });
        let subscriptions = vec![
            cx.subscribe(&composer, |this: &mut Self, _, event, cx| {
                if let ComposerEvent::Send(query) = event {
                    if this.active {
                        this.search(query, cx);
                    }
                }
            }),
            cx.observe_global::<TextScale>(|this: &mut Self, cx| {
                this.list.remeasure();
                cx.notify();
            }),
            // 休眠可能暂停后台计时器；重新激活时按查询完成的实际时间检查。
            cx.observe_window_activation(window, |this: &mut Self, window, cx| {
                if window.is_window_active() {
                    this.expire(cx);
                }
            }),
        ];
        Self {
            engine,
            composer,
            state: SearchState::default(),
            list: ListState::new(0, ListAlignment::Top, px(m::SPACE_12 * 4.0)),
            expanded: HashSet::new(),
            selected: None,
            active: false,
            back: cx.focus_handle().tab_stop(true),
            results_focus: cx.focus_handle().tab_stop(true),
            drag_source: source,
            query_task: None,
            expiry_task: None,
            locating: false,
            _subscriptions: subscriptions,
        }
    }

    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.active = active;
        self.composer
            .update(cx, |composer, cx| composer.set_enabled(active, cx));
        self.expire(cx);
        cx.notify();
    }

    pub fn focus_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.composer.focus_handle(cx), cx);
    }

    pub fn set_locating(&mut self, locating: bool, error: Option<String>, cx: &mut Context<Self>) {
        self.locating = locating;
        self.state.error = error;
        cx.notify();
    }

    fn reset_list(&mut self) {
        self.list.reset(self.state.results.len());
        self.expanded.clear();
        self.selected = None;
    }

    fn expire(&mut self, cx: &mut Context<Self>) {
        if self.state.expire(SystemTime::now()) {
            self.reset_list();
            cx.notify();
        }
    }

    fn search(&mut self, query: &str, cx: &mut Context<Self>) {
        if self.locating {
            return;
        }
        let Some(generation) = self.state.begin(query) else {
            return;
        };
        self.expiry_task = None;
        self.reset_list();
        let engine = self.engine.clone();
        let query = self.state.query.clone();
        let read = spawn_engine(cx, async move { engine.search_history(query).await });
        self.query_task = Some(cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |this, cx| {
                if !this.state.complete(generation, result, SystemTime::now()) {
                    return;
                }
                this.reset_list();
                if this.state.completed_at.is_some() {
                    this.expiry_task = Some(cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(RESULT_LIFETIME).await;
                        let _ = this.update(cx, |this, cx| this.expire(cx));
                    }));
                }
                this.query_task = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn locate(&mut self, result: HistoryMatch, cx: &mut Context<Self>) {
        if !self.active || self.locating {
            return;
        }
        self.selected = Some(result.message_id().to_owned());
        cx.emit(SearchEvent::Locate(result));
        cx.notify();
    }

    fn toggle_expanded(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.expanded.remove(id) {
            self.expanded.insert(id.to_owned());
        }
        if let Some(ix) = self.state.results.iter().position(|r| r.message_id() == id) {
            self.list.remeasure_items(ix..ix + 1);
        }
        cx.notify();
    }

    fn render_result(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some(result) = self.state.results.get(ix).cloned() else {
            return div().into_any_element();
        };
        let c = cx.buddy_theme().colors;
        let id = result.message_id().to_owned();
        let expanded = self.expanded.contains(&id);
        let first_line = result.question().lines().next().unwrap_or("").to_owned();
        let font_size = px(TextScale::body(cx));
        let line = window.text_system().shape_line(
            first_line.clone().into(),
            font_size,
            &[gpui::TextRun {
                len: first_line.len(),
                font: window.text_style().font(),
                color: c.text_primary.into(),
                ..Default::default()
            }],
            None,
        );
        // 预留展开按钮的槽位，宽度随窗口变化重新测量，不按字符数猜测中文折行。
        let can_expand = result.question().contains('\n')
            || line.width
                > (window.viewport_size().width - px(m::SPACE_4 * 2.0 + m::SPACE_12 + m::SPACE_2))
                    .max(px(0.));
        let question = if expanded {
            result.question().to_owned()
        } else {
            first_line
        };
        let expand_id = id.clone();
        let open = result.clone();
        div()
            .id(SharedString::from(format!("history-result-{id}")))
            .px(px(m::SPACE_4))
            .py(px(m::SPACE_3))
            .w_full()
            .border_b_1()
            .border_color(c.border_subtle)
            .when(self.selected.as_deref() == Some(&id), |d| {
                d.bg(c.primary_tint_soft)
            })
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(m::SPACE_2))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(font_size)
                            .text_color(c.text_primary)
                            .when(!expanded, |d| {
                                d.whitespace_nowrap().text_ellipsis().overflow_hidden()
                            })
                            .child(SharedString::from(question)),
                    )
                    .when(can_expand, |d| {
                        d.child(
                            div()
                                .id(SharedString::from(format!("expand-{id}")))
                                .flex_none()
                                .text_size(px(m::FONT_SIZE_SM))
                                .text_color(c.text_muted)
                                .rounded(px(m::RADIUS_SM))
                                .px(px(m::SPACE_1))
                                .when(self.active, |d| {
                                    d.hover(|s| s.bg(c.bg_sunken).text_color(c.text_primary))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.toggle_expanded(&expand_id, cx);
                                            cx.stop_propagation();
                                        }))
                                })
                                .child(if expanded { "收起" } else { "展开" }),
                        )
                    }),
            )
            .child(
                div()
                    .mt(px(m::SPACE_1))
                    .text_size(px(m::FONT_SIZE_XS))
                    .text_color(c.text_tertiary)
                    .child(format_time(result.created_at())),
            )
            .child(
                div()
                    .id(SharedString::from(format!("history-answer-{id}")))
                    .debug_selector(move || format!("history-answer-{ix}"))
                    .mt(px(m::SPACE_2))
                    .py(px(m::SPACE_2))
                    .rounded(px(m::RADIUS_MD))
                    .text_size(px(TextScale::secondary(cx)))
                    .line_height(gpui::relative(1.5))
                    .text_color(c.text_muted)
                    .when(self.active, |d| {
                        d.hover(|s| s.bg(c.bg_sunken).text_color(c.text_primary))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.locate(open.clone(), cx)),
                            )
                    })
                    // GPUI 的 line_clamp 只限制自动折行；显式换行还需限制可见高度。
                    .child(
                        div()
                            .line_clamp(2)
                            .max_h(px(TextScale::secondary(cx) * 3.0))
                            .overflow_hidden()
                            .child(SharedString::from(if result.answer().is_empty() {
                                "暂无回答".into()
                            } else {
                                result.answer().to_owned()
                            })),
                    ),
            )
            .into_any_element()
    }
}

impl Render for HistorySearch {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.buddy_theme().colors;
        let header = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(m::SPACE_2))
            .px(px(m::SPACE_4))
            .py(px(m::SPACE_3))
            .child(
                icon_button(
                    "history-back",
                    IconName::ArrowLeft,
                    m::SPACE_8,
                    m::FONT_SIZE_MD,
                    IconButtonVariant::Default,
                    !self.active,
                    cx,
                )
                .debug_selector(|| "history-back".into())
                .when(self.active, |d| {
                    d.track_focus(&self.back)
                        .focus_visible(|s| s.bg(c.primary_tint_soft))
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(SearchEvent::Back)))
                })
                .tooltip(|_, cx| TextTooltip::view(format!("返回对话 · {SHORTCUT_LABEL}"), cx)),
            )
            .child(
                div()
                    .text_size(px(m::FONT_SIZE_MD))
                    .text_color(c.text_primary)
                    .child("搜索历史"),
            )
            .child(
                drag::region_if(&self.drag_source, self.active)
                    .flex_1()
                    .h(px(m::SPACE_8)),
            );
        let status = if self.locating {
            Some("正在定位消息…".to_owned())
        } else if self.state.loading {
            Some("正在查询…".to_owned())
        } else if let Some(error) = &self.state.error {
            Some(format!("{error}，请重试"))
        } else if self.state.completed_at.is_some() {
            Some(format!(
                "“{}” · {} 个问题",
                self.state.query,
                self.state.results.len()
            ))
        } else {
            None
        };
        let results = div()
            .id("history-results")
            .flex_1()
            .min_h_0()
            .w_full()
            .when(self.active, |d| d.track_focus(&self.results_focus))
            .child(
                list(
                    self.list.clone(),
                    cx.processor(|this, ix, window, cx| this.render_result(ix, window, cx)),
                )
                .flex_grow(1.)
                .size_full(),
            );
        div()
            .id("history-search-page")
            .debug_selector(|| "history-search-page".into())
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(m::RADIUS_XL))
            .border_1()
            .border_color(c.window_outline)
            .bg(c.bg_surface)
            .when(self.active, |d| {
                d.occlude().on_key_down(cx.listener(
                    |this, event: &gpui::KeyDownEvent, window, cx| {
                        match event.keystroke.key.as_str() {
                            "escape" => cx.emit(SearchEvent::Back),
                            "tab" => {
                                let input = this.composer.focus_handle(cx);
                                let order = [&this.back, &this.results_focus, &input];
                                let ix = order
                                    .iter()
                                    .position(|focus| focus.is_focused(window))
                                    .unwrap_or(2);
                                let next =
                                    (ix + if event.keystroke.modifiers.shift {
                                        2
                                    } else {
                                        1
                                    }) % 3;
                                window.focus(order[next], cx);
                            }
                            "enter" | "space" if this.back.is_focused(window) => {
                                cx.emit(SearchEvent::Back)
                            }
                            "down" | "up" if this.results_focus.is_focused(window) => {
                                let count = this.state.results.len();
                                if count > 0 {
                                    let current = this.state.results.iter().position(|r| {
                                        Some(r.message_id()) == this.selected.as_deref()
                                    });
                                    let ix = current.map_or(0, |ix| {
                                        if event.keystroke.key == "down" {
                                            (ix + 1).min(count - 1)
                                        } else {
                                            ix.saturating_sub(1)
                                        }
                                    });
                                    this.selected =
                                        Some(this.state.results[ix].message_id().to_owned());
                                    this.list.scroll_to_reveal_item(ix);
                                    cx.notify();
                                }
                            }
                            "right" | "left" if this.results_focus.is_focused(window) => {
                                if let Some(id) = this.selected.clone() {
                                    this.toggle_expanded(&id, cx);
                                }
                            }
                            "enter" | "space" if this.results_focus.is_focused(window) => {
                                let found = this
                                    .state
                                    .results
                                    .iter()
                                    .find(|r| Some(r.message_id()) == this.selected.as_deref())
                                    .or(this.state.results.first())
                                    .cloned();
                                if let Some(result) = found {
                                    this.locate(result, cx);
                                }
                            }
                            _ => return,
                        }
                        cx.stop_propagation();
                    },
                ))
            })
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(gpui::linear_gradient(
                        145.,
                        gpui::linear_color_stop(c.surface_highlight, 0.),
                        gpui::linear_color_stop(
                            gpui::Hsla::from(c.surface_highlight).opacity(0.),
                            0.38,
                        ),
                    )),
            )
            .child(header)
            .when_some(status, |d, status| {
                d.child(
                    div()
                        .px(px(m::SPACE_4))
                        .pb(px(m::SPACE_2))
                        .text_size(px(m::FONT_SIZE_XS))
                        .text_color(c.text_tertiary)
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .overflow_hidden()
                        .child(status),
                )
            })
            .child(results)
            .child(self.composer.clone())
    }
}
