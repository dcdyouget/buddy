//! Shared window-drag callback for chat content and shell edge strips.

use gpui::{Div, MouseButton, Window, div, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;

/// The operation used to begin moving the native window.
pub type DragFn = Rc<dyn Fn(&mut Window)>;

/// A mutable indirection keeps self-tests able to replace the callback after
/// the transcript entity has been created, while production uses the native
/// `Window::start_window_move` operation.
pub type DragSource = Rc<RefCell<DragFn>>;

pub fn default_drag_source() -> DragSource {
    Rc::new(RefCell::new(Rc::new(|window: &mut Window| {
        window.start_window_move();
    })))
}

pub fn invoke(source: &DragSource, window: &mut Window) {
    let drag = source.borrow().clone();
    drag(window);
}

/// A transparent structural drag region. It must sit below interactive/text
/// children so those children retain their own hit testing and scrolling.
pub fn region(source: &DragSource) -> Div {
    region_if(source, true)
}

/// Keep structural spacing while an outgoing panel stops receiving input.
pub fn region_if(source: &DragSource, enabled: bool) -> Div {
    let source = source.clone();
    div().when(enabled, |d| {
        d.on_mouse_down(MouseButton::Left, move |_, window, cx| {
            cx.stop_propagation();
            invoke(&source, window);
        })
    })
}
