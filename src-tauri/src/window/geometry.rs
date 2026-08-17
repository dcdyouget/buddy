#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkArea {
    pub position: WindowPosition,
    pub size: WindowSize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetGeometry {
    pub position: WindowPosition,
    pub size: WindowSize,
}

const WINDOW_MARGIN_LOGICAL: f64 = 12.0;

fn logical_page_size(page: &str) -> Option<WindowSize> {
    match page {
        "empty" | "noapikey" => Some(WindowSize {
            width: 560,
            height: 60,
        }),
        "conversation" | "streaming" => Some(WindowSize {
            width: 750,
            height: 500,
        }),
        "settings" | "add-provider" => Some(WindowSize {
            width: 760,
            height: 640,
        }),
        _ => None,
    }
}

pub fn physical_page_size(page: &str, scale_factor: f64) -> Option<WindowSize> {
    let logical = logical_page_size(page)?;
    Some(WindowSize {
        width: (logical.width as f64 * scale_factor).round() as u32,
        height: (logical.height as f64 * scale_factor).round() as u32,
    })
}

pub fn physical_window_margin(scale_factor: f64) -> i32 {
    (WINDOW_MARGIN_LOGICAL * scale_factor).round() as i32
}

fn clamp_axis(value: i64, origin: i32, extent: u32, target: u32, margin: i32) -> i32 {
    let min = origin as i64 + margin as i64;
    let max = (origin as i64 + extent as i64 - target as i64 - margin as i64).max(min);
    value.clamp(min, max) as i32
}

pub fn calculate_bottom_anchored_target_geometry(
    start_position: WindowPosition,
    start_size: WindowSize,
    target_size: WindowSize,
    work_area: Option<WorkArea>,
    margin: i32,
) -> TargetGeometry {
    let anchored_x = (start_position.x as f64
        + (start_size.width as f64 - target_size.width as f64) / 2.0)
        .round() as i64;
    let anchored_y = start_position.y as i64 + start_size.height as i64 - target_size.height as i64;

    let position = match work_area {
        Some(work_area) => WindowPosition {
            x: clamp_axis(
                anchored_x,
                work_area.position.x,
                work_area.size.width,
                target_size.width,
                margin,
            ),
            y: clamp_axis(
                anchored_y,
                work_area.position.y,
                work_area.size.height,
                target_size.height,
                margin,
            ),
        },
        None => WindowPosition {
            x: anchored_x as i32,
            y: anchored_y as i32,
        },
    };

    TargetGeometry {
        position,
        size: target_size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_compact_bottom_and_horizontal_center_while_expanding() {
        let result = calculate_bottom_anchored_target_geometry(
            WindowPosition { x: 500, y: 400 },
            WindowSize {
                width: 560,
                height: 60,
            },
            WindowSize {
                width: 750,
                height: 500,
            },
            None,
            12,
        );

        assert_eq!(result.position, WindowPosition { x: 405, y: -40 });
    }

    #[test]
    fn keeps_expanded_window_inside_monitor_work_area() {
        let result = calculate_bottom_anchored_target_geometry(
            WindowPosition { x: 20, y: 30 },
            WindowSize {
                width: 460,
                height: 78,
            },
            WindowSize {
                width: 750,
                height: 500,
            },
            Some(WorkArea {
                position: WindowPosition { x: 0, y: 25 },
                size: WindowSize {
                    width: 1440,
                    height: 875,
                },
            }),
            12,
        );

        assert_eq!(result.position, WindowPosition { x: 12, y: 37 });
    }

    #[test]
    fn scales_page_size_and_margin_for_retina_monitor() {
        assert_eq!(
            physical_page_size("conversation", 2.0),
            Some(WindowSize {
                width: 1500,
                height: 1000,
            })
        );
        assert_eq!(physical_window_margin(2.0), 24);
    }

    #[test]
    fn rejects_unknown_page() {
        assert_eq!(physical_page_size("unknown", 2.0), None);
    }
}
