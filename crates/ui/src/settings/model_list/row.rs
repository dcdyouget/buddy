use super::ModelListView;
use super::helpers::{check_control, default_button, latency, model_info};
use crate::{
    components::TextTooltip,
    icons::{IconName, icon},
    settings::model_config::ModelEdit,
    theme_system::{BuddyTheme, tokens::metrics as m},
};
use buddy_engine::models::ModelInfo;
use gpui::{Context, KeyDownEvent, canvas, div, prelude::*, px};

fn activate(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
}

pub(super) fn render_row(
    this: &mut ModelListView,
    model: ModelInfo,
    interactive: bool,
    bounds: std::rc::Rc<
        std::cell::RefCell<std::collections::BTreeMap<String, gpui::Bounds<gpui::Pixels>>>,
    >,
    cx: &mut Context<ModelListView>,
) -> gpui::Stateful<gpui::Div> {
    let c = cx.buddy_theme().colors;
    let id = model.id.clone();
    let enabled = this.model_enabled(&id);
    let is_default = this.config.selected_model_id == id;
    let provider_name = this
        .config
        .providers
        .iter()
        .find(|p| p.id == model.provider_id)
        .map(|p| p.name.clone());
    let can_generate = this
        .provider_type(&id)
        .map(|kind| kind == "openai_compatible")
        .unwrap_or(false);
    let focus = this.model_focus(&id).cloned().expect("model focus");

    let model_id = id.clone();
    let enabled_control = check_control(
        &format!("enable-{id}"),
        enabled,
        interactive,
        m::SPACE_2 * 2.0,
        focus.enabled.clone(),
        cx.listener(move |this, _, window, cx| {
            window.focus(
                &this.model_focus(&model_id).expect("model focus").enabled,
                cx,
            );
            if this.active && !this.saving {
                cx.emit(ModelEdit::ToggleEnabled(model_id.clone()));
            }
        }),
        cx.listener({
            let model_id = id.clone();
            move |this, event, _, cx| {
                if this.active && !this.saving && activate(event) {
                    cx.emit(ModelEdit::ToggleEnabled(model_id.clone()));
                    cx.stop_propagation();
                }
            }
        }),
        bounds.clone(),
        cx,
    );

    let model_id = id.clone();
    let vision_control = check_control(
        &format!("vision-{id}"),
        model.supports_vision,
        interactive,
        m::FONT_SIZE_MD,
        focus.vision.clone(),
        cx.listener(move |this, _, window, cx| {
            window.focus(
                &this.model_focus(&model_id).expect("model focus").vision,
                cx,
            );
            if this.active && !this.saving {
                let value = !this.model(&model_id).is_some_and(|m| m.supports_vision);
                cx.emit(ModelEdit::SetVision(model_id.clone(), value));
            }
        }),
        cx.listener({
            let model_id = id.clone();
            move |this, event, _, cx| {
                if this.active && !this.saving && activate(event) {
                    let value = !this.model(&model_id).is_some_and(|m| m.supports_vision);
                    cx.emit(ModelEdit::SetVision(model_id.clone(), value));
                    cx.stop_propagation();
                }
            }
        }),
        bounds.clone(),
        cx,
    )
    .child("支持图片");

    let model_id = id.clone();
    let image_control = check_control(
        &format!("image-{id}"),
        can_generate && model.supports_image_generation,
        interactive && can_generate,
        m::FONT_SIZE_MD,
        focus.image.clone(),
        cx.listener(move |this, _, window, cx| {
            window.focus(&this.model_focus(&model_id).expect("model focus").image, cx);
            if this.active
                && !this.saving
                && this.provider_type(&model_id) == Some("openai_compatible")
            {
                let value = !this
                    .model(&model_id)
                    .is_some_and(|m| m.supports_image_generation);
                cx.emit(ModelEdit::SetImageGeneration(model_id.clone(), value));
            }
        }),
        cx.listener({
            let model_id = id.clone();
            move |this, event, _, cx| {
                if this.active
                    && !this.saving
                    && this.provider_type(&model_id) == Some("openai_compatible")
                    && activate(event)
                {
                    let value = !this
                        .model(&model_id)
                        .is_some_and(|m| m.supports_image_generation);
                    cx.emit(ModelEdit::SetImageGeneration(model_id.clone(), value));
                    cx.stop_propagation();
                }
            }
        }),
        bounds.clone(),
        cx,
    )
    .child("支持生图");
    let image_control = if can_generate {
        image_control
    } else {
        image_control.tooltip(|_, cx| TextTooltip::view("Anthropic 协议暂不支持图片生成", cx))
    };

    let context = this.contexts.get(&id).cloned().map(|select| {
        div()
            .id(format!("context-{id}"))
            .relative()
            .flex_none()
            .w(px(m::SPACE_8 * 2.0))
            .child(select)
            .child(
                canvas(|_, _, _| {}, {
                    let bounds = bounds.clone();
                    let id = id.clone();
                    move |rect, _, _, _| {
                        bounds.borrow_mut().insert(format!("context-{id}"), rect);
                    }
                })
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
    });

    let default_control = if !is_default && enabled {
        Some(default_button(
            &id,
            interactive,
            focus.default.clone(),
            bounds.clone(),
            cx,
        ))
    } else {
        None
    };
    let row_opacity = if enabled { 1.0 } else { 0.5 };
    let row_bounds = bounds.clone();
    let row_id = id.clone();
    div()
        .id(format!("model-row-{id}"))
        .relative()
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .py(px(m::SPACE_2))
        .border_t_1()
        .border_color(c.border_subtle)
        .opacity(row_opacity)
        .child(enabled_control)
        .child(
            div()
                .size(px(m::SPACE_6))
                .flex_none()
                .rounded(px(m::RADIUS_MD))
                .bg(c.bg_sunken)
                .flex()
                .items_center()
                .justify_center()
                .text_color(c.text_muted)
                .child(icon(IconName::Bot, px(m::FONT_SIZE_MD))),
        )
        .child(model_info(&model, provider_name, is_default, context, *c))
        .child(latency(model.latency_ms, *c))
        .child(vision_control)
        .child(image_control)
        .children(default_control)
        .child(
            canvas(
                |_, _, _| {},
                move |rect, _, _, _| {
                    row_bounds
                        .borrow_mut()
                        .insert(format!("row-{row_id}"), rect);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
}
