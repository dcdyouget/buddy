use super::*;

fn display(key: &str, frame: Rect, work_area: Rect, primary: bool) -> Screen {
    Screen {
        key: key.into(),
        frame,
        work_area,
        scale: 1.0,
        primary,
    }
}

#[test]
fn appkit_logical_scale_keeps_geometry_in_points() {
    let mut retina = display(
        "retina",
        Rect::new(0.0, 0.0, 1440.0, 900.0),
        Rect::new(0.0, 23.0, 1440.0, 877.0),
        true,
    );
    retina.scale = 1.5;
    let mut ultra = retina.clone();
    ultra.scale = 2.0;
    let size = Size {
        width: 750.0,
        height: 500.0,
    };
    assert_eq!(retina.scale, 1.5);
    assert_eq!(ultra.scale, 2.0);
    assert_eq!(
        PositionMemory::default().restore(&retina, size),
        PositionMemory::default().restore(&ultra, size)
    );
    let start = Rect::new(300.0, 200.0, 560.0, 60.0);
    assert_eq!(
        bottom_anchored(start, size, &retina),
        bottom_anchored(start, size, &ultra)
    );
}

#[test]
fn appkit_coordinate_conversion_round_trips() {
    let appkit = Rect::new(-1440.0, 120.0, 750.0, 500.0);
    let top_left = appkit_to_top_left(appkit, 1080.0);
    assert_eq!(
        top_left.origin,
        Point {
            x: -1440.0,
            y: 460.0
        }
    );
    assert_eq!(top_left_to_appkit(top_left, 1080.0), appkit);
}

#[test]
fn source_selection_follows_v1_fallback_order() {
    let screens = vec![
        display(
            "primary",
            Rect::new(0.0, 0.0, 1440.0, 900.0),
            Rect::new(0.0, 23.0, 1440.0, 877.0),
            true,
        ),
        display(
            "left",
            Rect::new(-1920.0, 0.0, 1920.0, 1080.0),
            Rect::new(-1920.0, 23.0, 1920.0, 1057.0),
            false,
        ),
    ];
    assert_eq!(
        choose_display(
            &screens,
            PositionSource::Focused,
            Some("missing"),
            Some("left"),
            None,
        )
        .unwrap()
        .key,
        "left"
    );
    assert_eq!(
        choose_display(
            &screens,
            PositionSource::Cursor,
            Some("missing"),
            Some("missing"),
            Some("left"),
        )
        .unwrap()
        .key,
        "left"
    );
    assert_eq!(
        choose_display(&screens, PositionSource::Focused, None, None, None)
            .unwrap()
            .key,
        "primary"
    );
}

#[test]
fn negative_display_origin_is_preserved_when_clamping() {
    let area = Rect::new(-1920.0, 23.0, 1920.0, 1057.0);
    let origin = clamp_to_work_area(
        Point {
            x: -2500.0,
            y: -100.0,
        },
        Size {
            width: 750.0,
            height: 500.0,
        },
        area,
        12.0,
    );
    assert_eq!(
        origin,
        Point {
            x: -1920.0 + 12.0,
            y: 23.0 + 12.0
        }
    );
    let centered = centered_target(
        &display("left", Rect::new(-1920.0, 0.0, 1920.0, 1080.0), area, false),
        Size {
            width: 750.0,
            height: 500.0,
        },
    );
    assert_eq!(
        centered.origin,
        Point {
            x: -1335.0,
            y: 290.0
        }
    );
}

#[test]
fn bottom_anchor_keeps_bottom_and_centers_when_inside_work_area() {
    let start = Rect::new(500.0, 400.0, 560.0, 60.0);
    let target = bottom_anchored_target(
        start,
        Size {
            width: 750.0,
            height: 500.0,
        },
        None,
        12.0,
    );
    assert_eq!(target.origin, Point { x: 405.0, y: -40.0 });
    let clipped = bottom_anchored_target(
        start,
        Size {
            width: 750.0,
            height: 500.0,
        },
        Some(Rect::new(0.0, 25.0, 1440.0, 875.0)),
        12.0,
    );
    assert_eq!(clipped.origin, Point { x: 405.0, y: 37.0 });
}

#[test]
fn oversized_window_keeps_full_frame_centering_for_show() {
    let area = Rect::new(0.0, 0.0, 400.0, 300.0);
    let target = show_target(
        &display("small", area, area, false),
        Size {
            width: 750.0,
            height: 500.0,
        },
        None,
    );
    assert_eq!(
        target.origin,
        Point {
            x: -175.0,
            y: -100.0
        }
    );
}

#[test]
fn invalid_saved_position_falls_back_to_full_frame_center() {
    let display = display(
        "primary",
        Rect::new(0.0, 0.0, 1440.0, 900.0),
        Rect::new(0.0, 23.0, 1440.0, 877.0),
        true,
    );
    let target = show_target(
        &display,
        Size {
            width: 750.0,
            height: 500.0,
        },
        Some(Point {
            x: 1200.0,
            y: 500.0,
        }),
    );
    assert_eq!(target.origin, Point { x: 345.0, y: 200.0 });
}

#[test]
fn position_memory_saves_per_screen_and_restores_valid_frame_position() {
    let mut memory = PositionMemory::default();
    let screen = display(
        "screen",
        Rect::new(-100.0, 0.0, 800.0, 600.0),
        Rect::new(-100.0, 20.0, 800.0, 580.0),
        false,
    );
    memory.save(&screen, Point { x: 300.0, y: 300.0 });
    assert_eq!(
        memory.restore(
            &screen,
            Size {
                width: 200.0,
                height: 100.0
            }
        ),
        Point { x: 300.0, y: 300.0 }
    );
    let other = display(
        "other",
        Rect::new(0.0, 0.0, 800.0, 600.0),
        Rect::new(0.0, 20.0, 800.0, 580.0),
        true,
    );
    assert_eq!(
        memory.restore(
            &other,
            Size {
                width: 200.0,
                height: 100.0
            }
        ),
        Point { x: 300.0, y: 250.0 } // (full-frame 600 - window 100) / 2
    );
}

#[test]
fn page_resize_clamps_right_bottom_and_oversized_targets() {
    let target = bottom_anchored_target(
        Rect::new(2000.0, 2000.0, 560.0, 60.0),
        Size {
            width: 750.0,
            height: 500.0,
        },
        Some(Rect::new(0.0, 25.0, 1440.0, 875.0)),
        12.0,
    );
    assert_eq!(target.origin, Point { x: 678.0, y: 388.0 });
    let oversized = bottom_anchored_target(
        Rect::new(100.0, 100.0, 200.0, 60.0),
        Size {
            width: 750.0,
            height: 500.0,
        },
        Some(Rect::new(-500.0, 25.0, 400.0, 300.0)),
        12.0,
    );
    assert_eq!(oversized.origin, Point { x: -488.0, y: 37.0 });
}
