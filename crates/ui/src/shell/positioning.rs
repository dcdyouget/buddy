//! 主窗口定位的纯几何与运行期位置记忆（S07-06）。
//!
//! 坐标统一使用全局逻辑点：原点在主屏左上，其他屏幕可以有负坐标。
//! 原生桥负责把 AppKit 的左下坐标转换为这些坐标；本模块不接触 GPUI 或 AppKit，
//! 便于在没有多屏硬件的环境中覆盖所有边界分支。

use std::collections::HashMap;

use crate::theme_system::tokens::metrics as m;
/// 页面尺寸与工作区边缘之间的最小间距（逻辑点）。
pub const WINDOW_MARGIN: f64 = m::SPACE_3 as f64;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
/// 全局逻辑坐标中的点。
pub struct Point {
    /// 横坐标，允许为负。
    pub x: f64,
    /// 纵坐标，原点在主屏左上。
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
/// 窗口或显示区域的逻辑尺寸。
pub struct Size {
    /// 宽度。
    pub width: f64,
    /// 高度。
    pub height: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
/// 全局逻辑坐标中的矩形。
pub struct Rect {
    /// 左上角原点。
    pub origin: Point,
    /// 矩形尺寸。
    pub size: Size,
}

impl Rect {
    /// 创建一个全局逻辑矩形。
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            origin: Point { x, y },
            size: Size { width, height },
        }
    }
}

/// 将 AppKit 左下原点矩形转换为统一的全局左上原点矩形。
pub fn appkit_to_top_left(rect: Rect, primary_top: f64) -> Rect {
    Rect::new(
        rect.origin.x,
        primary_top - (rect.origin.y + rect.size.height),
        rect.size.width,
        rect.size.height,
    )
}

/// 将统一的全局左上原点矩形转换为 AppKit 左下原点矩形。
pub fn top_left_to_appkit(rect: Rect, primary_top: f64) -> Rect {
    appkit_to_top_left(rect, primary_top)
}

/// 一个显示器的稳定标识与工作区几何。
#[derive(Clone, Debug, PartialEq)]
/// 当前显示器的 frame、工作区和缩放信息。
pub struct Screen {
    /// 优先使用 CGDisplay UUID；非 macOS 测试可使用任意稳定 key。
    pub key: String,
    /// 全屏 frame，允许负坐标。
    pub frame: Rect,
    /// 排除 Dock / 菜单栏后的可放置区域。
    pub work_area: Rect,
    /// backing scale，用于逻辑点与物理像素换算。
    pub scale: f64,
    /// 是否为主显示器。
    pub primary: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// 呼出动作选择目标显示器的来源。
pub enum PositionSource {
    /// 全局热键：优先当前键盘焦点窗口所在屏幕。
    Focused,
    /// 托盘点击：优先鼠标所在屏幕。
    Cursor,
}

/// 按 v1 的三层回退选择屏幕；`current_key` 是主窗口当前所在屏幕。
pub fn choose_display<'a>(
    displays: &'a [Screen],
    source: PositionSource,
    focused_key: Option<&str>,
    cursor_key: Option<&str>,
    current_key: Option<&str>,
) -> Option<&'a Screen> {
    let candidates = match source {
        PositionSource::Focused => [focused_key, cursor_key, current_key],
        PositionSource::Cursor => [cursor_key, current_key, None],
    };
    candidates
        .into_iter()
        .flatten()
        .find_map(|key| displays.iter().find(|display| display.key == key))
        .or_else(|| displays.iter().find(|display| display.primary))
        .or_else(|| displays.first())
}

fn clamp_axis(value: f64, start: f64, extent: f64, target: f64, margin: f64) -> f64 {
    let min = start + margin;
    let max = (start + extent - target - margin).max(min);
    value.clamp(min, max)
}

/// 将窗口原点裁剪到工作区；窗口大于工作区时仍返回工作区内的最小可行原点。
pub fn clamp_to_work_area(origin: Point, size: Size, work_area: Rect, margin: f64) -> Point {
    Point {
        x: clamp_axis(
            origin.x,
            work_area.origin.x,
            work_area.size.width,
            size.width,
            margin,
        ),
        y: clamp_axis(
            origin.y,
            work_area.origin.y,
            work_area.size.height,
            size.height,
            margin,
        ),
    }
}

/// 按 v1 在完整显示器 frame 内居中；工作区裁剪只用于页面 resize。
pub fn centered_target(display: &Screen, size: Size) -> Rect {
    let origin = Point {
        x: display.frame.origin.x + (display.frame.size.width - size.width) / 2.0,
        y: display.frame.origin.y + (display.frame.size.height - size.height) / 2.0,
    };
    Rect { origin, size }
}

fn fits_frame(origin: Point, size: Size, frame: Rect) -> bool {
    origin.x >= frame.origin.x
        && origin.y >= frame.origin.y
        && origin.x + size.width <= frame.origin.x + frame.size.width
        && origin.y + size.height <= frame.origin.y + frame.size.height
}

/// 呼出窗口：saved 完整位于该屏幕 frame 内才复用，否则回退完整 frame 居中。
pub fn show_target(display: &Screen, size: Size, saved_origin: Option<Point>) -> Rect {
    let origin = saved_origin
        .filter(|origin| fits_frame(*origin, size, display.frame))
        .unwrap_or_else(|| centered_target(display, size).origin);
    Rect { origin, size }
}

/// 页面增高时保持底边与横向中心；每个轴都按工作区裁剪。
pub fn bottom_anchored_target(
    start: Rect,
    target_size: Size,
    work_area: Option<Rect>,
    margin: f64,
) -> Rect {
    let origin = Point {
        x: start.origin.x + (start.size.width - target_size.width) / 2.0,
        y: start.origin.y + start.size.height - target_size.height,
    };
    let origin = work_area.map_or(origin, |area| {
        clamp_to_work_area(origin, target_size, area, margin)
    });
    Rect {
        origin,
        size: target_size,
    }
}

/// 与主 agent controller 约定的页面增高 API。
pub fn bottom_anchored(start: Rect, target_size: Size, screen: &Screen) -> Point {
    bottom_anchored_target(start, target_size, Some(screen.work_area), WINDOW_MARGIN).origin
}

/// 每屏位置只在本次运行期保存；磁盘配置不承担窗口瞬时几何。
#[derive(Debug, Default)]
pub struct PositionMemory {
    positions: HashMap<String, Point>,
}

impl PositionMemory {
    /// 读取某个显示器的当前运行期位置；未发生过移动时返回 `None`。
    pub fn saved(&self, display_key: &str) -> Option<Point> {
        self.positions.get(display_key).copied()
    }

    /// 记录拖动后的最新位置；160ms 防抖由外壳 controller 调度。
    pub fn save(&mut self, screen: &Screen, origin: Point) {
        self.positions.insert(screen.key.clone(), origin);
    }

    /// 恢复该屏位置；saved 不在完整 frame 内时回退到完整 frame 居中。
    pub fn restore(&self, screen: &Screen, size: Size) -> Point {
        show_target(screen, size, self.saved(&screen.key)).origin
    }
}

#[cfg(test)]
#[path = "positioning/tests.rs"]
mod tests;
