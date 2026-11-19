use std::sync::Arc;

use re_log_types::external::arrow::array::{
    Array as _, BooleanArray, Float64Array, Int32Array, ListArray, StringArray, StructArray,
};
use re_log_types::external::arrow::datatypes::{DataType, Field};
use re_log_types::{EntityPath, TimePoint, Timeline, TimelineName};
use re_sdk_types::archetypes::{self, Scalars, SeriesLines, SeriesPoints, Transform3D};
use re_sdk_types::blueprint;
use re_sdk_types::blueprint::archetypes::{ScalarAxis, TimeAxis, VisibleTimeRanges};
use re_sdk_types::encodings::{self, TimeRange};
use re_sdk_types::{DynamicArchetype, ToArrow as _, VisualizableArchetype as _, components};
use re_test_context::TestContext;
use re_test_context::VisualizerBlueprintContext as _;
use re_test_context::external::egui_kittest::{Harness, SnapshotResults, kittest::Queryable as _};
use re_test_viewport::TestContextExt as _;
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{BlueprintContext as _, ViewClass as _, ViewId};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

#[test]
fn double_clicking_scalar_axis_unsets_scalar_range() {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();
    let timeline = Timeline::log_tick();
    for (tick, value) in [(1, 2.0), (2, 4.0)] {
        test_context.log_entity("plot", |builder| {
            builder.with_archetype_auto_row([(timeline, tick)], &Scalars::single(value))
        });
    }
    test_context.set_active_timeline(*timeline.name());

    let fixed_range = components::Range1D::new(-10.0, 10.0);
    let view_id =
        test_context.setup_viewport_blueprint(|ctx, blueprint| {
            let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());
            ViewProperty::from_archetype_for_view::<ScalarAxis>(ctx, view.id)
                .save_blueprint_component(ctx, &ScalarAxis::descriptor_range(), &fixed_range);
            blueprint.add_view_at_root(view)
        });

    // Unsetting must also work when the default blueprint supplies a fixed range.
    let default_blueprint_id = test_context.active_blueprint().store_id().clone();
    let store_hub = test_context.store_hub.get_mut();
    store_hub
        .set_default_blueprint_for_app(&default_blueprint_id)
        .unwrap();
    store_hub
        .set_cloned_blueprint_active_for_app(&default_blueprint_id)
        .unwrap();

    let time_range = blueprint::components::TimeRange(TimeRange::from_cursor_plus_minus(10));
    test_context.with_blueprint_ctx(|ctx, _| {
        ViewProperty::from_archetype_for_view::<TimeAxis>(&ctx, view_id).save_blueprint_component(
            &ctx,
            &TimeAxis::descriptor_view_range(),
            &time_range,
        );
    });

    let size = egui::vec2(500.0, 300.0);
    let mut harness = test_context
        .setup_kittest_for_rendering_ui(size)
        .with_step_dt(1.0 / 60.0)
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));
    harness.run();
    assert_eq!(
        axis_ranges(&test_context, view_id),
        (Some(fixed_range), Some(time_range))
    );

    double_click_at(&mut harness, egui::pos2(12.0, size.y / 2.0));

    assert_eq!(
        axis_ranges(&test_context, view_id),
        (None, Some(time_range))
    );
}

#[test]
fn double_clicking_time_axis_fits_timeline_and_follows_cursor() {
    check_timeline_fit(
        egui::pos2(250.0, 286.0),
        Some(components::Range1D::new(-10.0, 10.0)),
    );
}

#[test]
fn double_clicking_plot_background_fits_both_axes() {
    check_timeline_fit(egui::pos2(400.0, 200.0), None);
}

fn check_timeline_fit(click: egui::Pos2, expected_y: Option<components::Range1D>) {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();
    let timeline = Timeline::new_sequence("tick");
    for tick in [100, 1200] {
        test_context.log_entity("plot", |builder| {
            builder.with_archetype_auto_row([(timeline, tick)], &Scalars::single(4.0))
        });
    }
    // The active timeline extends beyond the plotted series at both ends.
    for tick in [10, 3010] {
        test_context.log_entity("events", |builder| {
            builder.with_archetype_auto_row([(timeline, tick)], &archetypes::TextLog::new("event"))
        });
    }
    test_context.set_active_timeline(*timeline.name());
    test_context.set_time(310_i64);

    let fixed_y = components::Range1D::new(-10.0, 10.0);
    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());
        ctx.save_blueprint_archetype(
            ViewProperty::from_archetype_for_view::<ScalarAxis>(ctx, view.id).blueprint_store_path,
            &ScalarAxis::new().with_range(fixed_y),
        );
        ctx.save_blueprint_archetype(
            ViewProperty::from_archetype_for_view::<TimeAxis>(ctx, view.id).blueprint_store_path,
            &TimeAxis::new().with_view_range(TimeRange {
                start: encodings::TimeRangeBoundary::Absolute(200.into()),
                end: encodings::TimeRangeBoundary::Absolute(400.into()),
            }),
        );
        blueprint.add_view_at_root(view)
    });

    let size = egui::vec2(500.0, 300.0);
    let mut harness = test_context
        .setup_kittest_for_rendering_ui(size)
        .with_step_dt(1.0 / 60.0)
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));
    harness.run();
    double_click_at(&mut harness, click);
    harness.run();

    let fitted_time_range = TimeRange {
        start: encodings::TimeRangeBoundary::CursorRelative((-300).into()),
        end: encodings::TimeRangeBoundary::CursorRelative(2700.into()),
    };
    assert_eq!(
        axis_ranges(&test_context, view_id),
        (expected_y, Some(fitted_time_range.into()))
    );

    // As time advances, the window scrolls and the cursor stays at the same screen position.
    let cursor_label_x = harness.get_by_label("#310").rect().left();
    test_context.set_time(910_i64);
    harness.run();
    assert_eq!(harness.get_by_label("#910").rect().left(), cursor_label_x);
}

fn axis_ranges(
    test_context: &TestContext,
    view_id: ViewId,
) -> (
    Option<components::Range1D>,
    Option<blueprint::components::TimeRange>,
) {
    test_context.with_blueprint_ctx(|ctx, _| {
        let scalar = ViewProperty::from_archetype_for_view::<ScalarAxis>(&ctx, view_id);
        let time = ViewProperty::from_archetype_for_view::<TimeAxis>(&ctx, view_id);
        (
            scalar
                .component_or_empty::<components::Range1D>(ScalarAxis::descriptor_range().component)
                .expect("failed to read the scalar axis range"),
            time.component_or_empty::<blueprint::components::TimeRange>(
                TimeAxis::descriptor_view_range().component,
            )
            .expect("failed to read the time axis range"),
        )
    })
}

fn double_click_at(harness: &mut Harness<'_>, pos: egui::Pos2) {
    harness.hover_at(pos);
    for pressed in [true, false, true, false] {
        harness.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.step();
}

#[test]
fn shift_tap_and_drag_zooms_like_secondary_drag() {
    let zoom = |button, modifiers| {
        let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();
        let timeline = Timeline::log_tick();
        for tick in [0, 100] {
            test_context.log_entity("plot", |builder| {
                builder.with_archetype_auto_row([(timeline, tick)], &Scalars::single(tick as f64))
            });
        }
        test_context.set_active_timeline(*timeline.name());
        test_context.set_time(80_i64);
        let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
            blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
                TimeSeriesView::identifier(),
            ))
        });
        let mut harness = test_context
            .setup_kittest_for_rendering_ui(egui::vec2(500.0, 300.0))
            .with_step_dt(1.0 / 60.0)
            .build_ui(|ui| test_context.run_with_single_view(ui, view_id));
        harness.run();

        // Start at the time marker to also exercise the competition with cursor dragging.
        let start = egui::pos2(400.0, 100.0);
        let end = egui::pos2(150.0, 200.0);
        let button_event = |pos, pressed, modifiers| egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers,
        };
        harness.hover_at(start);
        harness.event(egui::Event::ModifiersChanged(modifiers));
        if button == egui::PointerButton::Primary {
            // A trackpad tap followed by a held second tap generates a click, then a press.
            harness.event(button_event(start, true, modifiers));
            harness.event(button_event(start, false, modifiers));
        }
        harness.event(button_event(start, true, modifiers));
        harness.hover_at(start + egui::vec2(-8.0, 4.0));
        harness.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        harness.hover_at(end);
        harness.step();
        assert_eq!(
            axis_ranges(&test_context, view_id),
            (None, None),
            "dragging should preview the box before applying the zoom"
        );
        if button == egui::PointerButton::Primary {
            harness.snapshot("shift_drag_box_zoom");
        }

        harness.event(button_event(end, false, egui::Modifiers::NONE));
        harness.run();
        assert_eq!(test_context.time_ctrl.read().time_i64(), Some(80));
        axis_ranges(&test_context, view_id)
    };

    let trackpad = zoom(egui::PointerButton::Primary, egui::Modifiers::SHIFT);
    let mouse = zoom(egui::PointerButton::Secondary, egui::Modifiers::NONE);
    assert!(
        trackpad.0.is_some() && trackpad.1.is_some(),
        "box zoom should set both axis ranges"
    );
    assert_eq!(trackpad, mouse);
}

#[test]
pub fn test_blueprint_overrides_and_defaults_with_time_series() {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();

    let timeline = Timeline::log_tick();

    log_data(&mut test_context, timeline);

    test_context.set_active_timeline(*timeline.name());

    let view_id = setup_blueprint(&mut test_context, timeline.name(), None, None);
    let size = egui::vec2(300.0, 300.0);
    let mut snapshot_results = SnapshotResults::new();
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "blueprint_overrides_and_defaults_with_time_series",
        size,
        None,
    ));

    for (range, name) in [
        (TimeRange::EVERYTHING, "everything"),
        (TimeRange::AT_CURSOR, "at_cursor"),
        (
            TimeRange {
                start: encodings::TimeRangeBoundary::CursorRelative(encodings::TimeInt(-10)),
                end: encodings::TimeRangeBoundary::CursorRelative(encodings::TimeInt(10)),
            },
            "around_cursor",
        ),
        (
            TimeRange {
                start: encodings::TimeRangeBoundary::Absolute(encodings::TimeInt(10)),
                end: encodings::TimeRangeBoundary::Absolute(encodings::TimeInt(20)),
            },
            "absolute",
        ),
        (
            TimeRange {
                start: encodings::TimeRangeBoundary::Absolute(encodings::TimeInt(10)),
                end: encodings::TimeRangeBoundary::Infinite,
            },
            "absolute_until_end",
        ),
        (
            TimeRange {
                start: encodings::TimeRangeBoundary::Infinite,
                end: encodings::TimeRangeBoundary::Absolute(encodings::TimeInt(15)),
            },
            "start_until_absolute",
        ),
    ] {
        let view_id = setup_blueprint(&mut test_context, timeline.name(), Some(range), None);
        snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
            view_id,
            &format!("blueprint_overrides_and_defaults_with_time_series_{name}"),
            size,
            None,
        ));
    }
}

#[test]
pub fn test_custom_visible_time_range() {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();

    let timeline = Timeline::log_tick();

    log_data(&mut test_context, timeline);

    test_context.set_active_timeline(*timeline.name());

    let size = egui::vec2(300.0, 300.0);

    let data_ranges = [
        (TimeRange::EVERYTHING, "everything"),
        (
            TimeRange {
                start: encodings::TimeRangeBoundary::CursorRelative(encodings::TimeInt(-10)),
                end: encodings::TimeRangeBoundary::CursorRelative(encodings::TimeInt(10)),
            },
            "around_cursor",
        ),
        (
            TimeRange {
                start: encodings::TimeRangeBoundary::Absolute(encodings::TimeInt(10)),
                end: encodings::TimeRangeBoundary::Absolute(encodings::TimeInt(20)),
            },
            "absolute",
        ),
        (
            TimeRange {
                start: encodings::TimeRangeBoundary::Absolute(encodings::TimeInt(10)),
                end: encodings::TimeRangeBoundary::Infinite,
            },
            "absolute_until_end",
        ),
        (
            TimeRange {
                start: encodings::TimeRangeBoundary::Infinite,
                end: encodings::TimeRangeBoundary::Absolute(encodings::TimeInt(15)),
            },
            "start_until_absolute",
        ),
    ];

    let mut snapshot_results = SnapshotResults::new();
    for (view_name, view_range) in [
        ("data", None),
        (
            "timeline",
            Some(TimeRange {
                start: encodings::TimeRangeBoundary::Absolute(encodings::TimeInt(0)),
                end: encodings::TimeRangeBoundary::Absolute(encodings::TimeInt(MAX_TIME)),
            }),
        ),
    ] {
        for (data_range, data_name) in &data_ranges {
            let view_id = setup_blueprint(
                &mut test_context,
                timeline.name(),
                view_range,
                Some(*data_range),
            );
            snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
                view_id,
                &format!("visible_time_range_{data_name}_view_{view_name}"),
                size,
                None,
            ));
        }
    }
}

const MAX_TIME: i64 = 31;

fn log_data(test_context: &mut TestContext, timeline: re_log_types::Timeline) {
    for i in 0..=MAX_TIME {
        let timepoint = TimePoint::from([(timeline, i)]);
        let t = i as f64 / 8.0;
        test_context.log_entity("plots/sin", |builder| {
            builder.with_archetype_auto_row(timepoint.clone(), &Scalars::single(t.sin()))
        });
        test_context.log_entity("plots/cos", |builder| {
            builder.with_archetype_auto_row(timepoint, &Scalars::single(t.cos()))
        });
    }
}

fn log_data_with_linear_speed(test_context: &mut TestContext, timeline: re_log_types::Timeline) {
    log_data(test_context, timeline);

    test_context.log_entity("plots/speed", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &SeriesLines::new()
                .with_colors([components::Color::from_rgb(255, 0, 0)])
                .with_names(["store_name"]),
        )
    });

    for i in 0..=MAX_TIME {
        let timepoint = TimePoint::from([(timeline, i)]);
        let t = i as f64 / 8.0;
        test_context.log_entity("plots/speed", |builder| {
            builder.with_archetype_auto_row(
                timepoint.clone(),
                &DynamicArchetype::new("custom")
                    .with_component::<components::LinearSpeed>("custom_component", [t.cos()]),
            )
        });
    }
}

fn setup_blueprint(
    test_context: &mut TestContext,
    timeline: &TimelineName,
    time_axis_view: Option<TimeRange>,
    visible_time_range: Option<TimeRange>,
) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());

        // Overrides:
        ctx.save_visualizers(
            &EntityPath::from("plots/cos"),
            view.id,
            // Override color and markers for the `cos` plot and make it a points visualizer.
            [&archetypes::SeriesPoints::default()
                .with_colors([(0, 255, 0)])
                .with_markers([components::MarkerShape::Cross])],
        );

        // Override default color (should apply to the `sin` plot).
        ctx.save_blueprint_archetype(
            view.defaults_path.clone(),
            &archetypes::SeriesLines::default().with_colors([(0, 0, 255)]),
        );

        if let Some(time_axis_view) = time_axis_view {
            let time_axis = re_viewport_blueprint::ViewProperty::from_archetype_for_view::<
                blueprint::archetypes::TimeAxis,
            >(ctx, view.id);

            time_axis.save_blueprint_component(
                ctx,
                &blueprint::archetypes::TimeAxis::descriptor_view_range(),
                &time_axis_view,
            );
        }

        if let Some(visible_time_range) = visible_time_range {
            let property = re_viewport_blueprint::ViewProperty::from_archetype_for_view::<
                VisibleTimeRanges,
            >(ctx, view.id);

            property.save_blueprint_component(
                ctx,
                &VisibleTimeRanges::descriptor_ranges(),
                &blueprint::components::VisibleTimeRange(encodings::VisibleTimeRange {
                    timeline: timeline.as_str().into(),
                    range: visible_time_range,
                }),
            );
        }

        blueprint.add_view_at_root(view)
    })
}

#[test]
pub fn test_explicit_component_mapping() {
    let mut test_context = TestContext::new();
    test_context.register_view_class::<TimeSeriesView>();

    let timeline = test_context.active_timeline().unwrap();
    log_data_with_linear_speed(&mut test_context, timeline);

    let view_id = setup_blueprint_with_explicit_mapping(&mut test_context);

    let size = egui::vec2(300.0, 300.0);
    let mut snapshot_results = SnapshotResults::new();
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "explicit_component_mapping",
        size,
        None,
    ));
}

fn setup_blueprint_with_explicit_mapping(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        use re_sdk_types::blueprint::encodings::{ComponentSourceKind, VisualizerComponentMapping};

        let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());

        // Single line visualizer for the `sin` plot:
        // * scalar - map explicitly to Scalar component.
        // * color - explicitly use default (blue), ignore provided override.
        // * .. everything else to auto
        ctx.save_visualizers(
            &EntityPath::from("plots/sin"),
            view.id,
            [SeriesLines::new()
                .with_colors([components::Color::from_rgb(255, 0, 255)])
                .visualizer()
                .with_mappings([
                    VisualizerComponentMapping {
                        target: Scalars::descriptor_scalars().component.as_str().into(),
                        source_kind: ComponentSourceKind::SourceComponent,
                        source_component: None,
                        selector: None,
                    }
                    .into(),
                    VisualizerComponentMapping {
                        target: SeriesLines::descriptor_colors().component.as_str().into(),
                        source_kind: ComponentSourceKind::Default,
                        source_component: None,
                        selector: None,
                    }
                    .into(),
                ])],
        );

        // Two visualizers for the `speed` plot:
        // * Points:
        //    * scalar - map to `LinearSpeed` component.
        //    * color - explicitly use Default, leading to the fallback since the view default is on lines.
        //    * … everything else is auto, which will not pick up anything from the store.
        // * Lines:
        //    * scalar - map to `LinearSpeed` component.
        //    * color - explicitly use Override
        //    * … everything else is auto, which will pick up the SeriesLines name from the store.
        let scalar_mapping = VisualizerComponentMapping {
            target: Scalars::descriptor_scalars().component.as_str().into(),
            source_kind: ComponentSourceKind::SourceComponent,
            source_component: Some("custom:custom_component".into()),
            selector: None,
        };
        ctx.save_visualizers(
            &EntityPath::from("plots/speed"),
            view.id,
            [
                SeriesPoints::new().visualizer().with_mappings([
                    blueprint::components::VisualizerComponentMapping(scalar_mapping.clone()),
                    blueprint::components::VisualizerComponentMapping(VisualizerComponentMapping {
                        target: SeriesPoints::descriptor_colors().component.as_str().into(),
                        source_kind: ComponentSourceKind::Default,
                        source_component: None,
                        selector: None,
                    }),
                ]),
                SeriesLines::new()
                    .with_colors([components::Color::from_rgb(0, 255, 0)])
                    .visualizer()
                    .with_mappings([
                        blueprint::components::VisualizerComponentMapping(scalar_mapping),
                        blueprint::components::VisualizerComponentMapping(
                            VisualizerComponentMapping {
                                target: SeriesLines::descriptor_colors().component.as_str().into(),
                                source_kind: ComponentSourceKind::Override,
                                source_component: None,
                                selector: None,
                            },
                        ),
                    ]),
            ],
        );

        // No visualization at all for plots/cos.
        ctx.save_visualizers(
            &EntityPath::from("plots/cos"),
            view.id,
            std::iter::empty::<re_sdk_types::Visualizer>(),
        );

        // Set a default color on the view (blue)
        ctx.save_blueprint_archetype(
            view.defaults_path.clone(),
            &archetypes::SeriesLines::default().with_colors([(0, 0, 255)]),
        );

        blueprint.add_view_at_root(view)
    })
}

#[test]
pub fn test_explicit_component_mapping_nested() {
    let mut test_context = TestContext::new();
    test_context.register_view_class::<TimeSeriesView>();

    let timeline = test_context.active_timeline().unwrap();
    log_data_nested(&mut test_context, timeline);

    let view_id = setup_blueprint_with_explicit_mapping_nested(&mut test_context);

    let size = egui::vec2(300.0, 300.0);
    let mut snapshot_results = SnapshotResults::new();
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "explicit_component_mapping_nested",
        size,
        None,
    ));
}

fn log_data_nested(test_context: &mut TestContext, timeline: re_log_types::Timeline) {
    for i in 0..=MAX_TIME {
        let sigmoid = |x: f64| 1.0 / (1.0 + (-x).exp());

        let timepoint = TimePoint::from([(timeline, i)]);
        let t = i as f64 / 8.0;

        // Flipped sigmoid scaled to [-2, 2]
        let single_value = (1.0 - sigmoid(t - 2.0)) * 4.0 - 2.0;

        let shifts = [0.0, -0.5, 0.5];
        let multiple_values: Vec<f64> = shifts
            .iter()
            .map(|&shift| (1.0 - sigmoid(t - 2.0 + shift)) * 4.0 - 2.0)
            .collect();

        use re_log_types::external::arrow;

        let offsets = arrow::buffer::OffsetBuffer::from_lengths([multiple_values.len()]);
        let values = Arc::new(Float64Array::from(multiple_values)) as Arc<dyn arrow::array::Array>;
        let list_array = ListArray::new(
            Arc::new(Field::new("item", DataType::Float64, false)),
            offsets,
            values,
            None,
        );

        // Create properties struct with color and name fields
        // Gradient from blue (0, 0, 255) to red (255, 0, 0)
        let t = i as f32 / MAX_TIME as f32; // Normalized time [0, 1]
        let r = 0u8;
        let g = (t * 255.0) as u8;
        let b = ((1.0 - t) * 255.0) as u8;
        let color = components::Color::from_rgb(r, g, b);

        // Serialize Color to Arrow using the `ToArrow` trait
        let color_arrow = encodings::Rgba32::to_arrow([color.0]).expect("could not convert color");
        let color_offsets = arrow::buffer::OffsetBuffer::from_lengths([1]);
        let color_list_array = ListArray::new(
            Arc::new(Field::new_list_field(DataType::UInt32, false)),
            color_offsets,
            color_arrow,
            None,
        );

        // We are going to build up something that looks like this:
        //
        // ```
        // Struct(
        //   single Float64,
        //   multiple List(Float64),
        //   stairstep Int32,
        //   toggle Boolean,
        //   properties Struct(
        //     color List(UInt32),
        //     name Utf8))
        // )
        // ```

        // Int32 staircase from -2 to 2 to test casting (each step lasts ~6 ticks)
        let stairstep_value = (i * 5 / (MAX_TIME + 1)) as i32 - 2;

        // Boolean square wave to test bool-to-scalar casting - toggles every 4 ticks
        let toggle_value = (i / 4) % 2 == 0;

        let name_array =
            Arc::new(StringArray::from(vec!["my-sigmoid"])) as Arc<dyn arrow::array::Array>;

        let properties_struct = StructArray::from(vec![
            (
                Arc::new(Field::new(
                    "color",
                    color_list_array.data_type().clone(),
                    false,
                )),
                Arc::new(color_list_array) as Arc<dyn arrow::array::Array>,
            ),
            (
                Arc::new(Field::new("name", DataType::Utf8, false)),
                name_array,
            ),
        ]);

        let struct_array = StructArray::from(vec![
            (
                Arc::new(Field::new("single", DataType::Float64, false)),
                Arc::new(Float64Array::from(vec![single_value])) as Arc<dyn arrow::array::Array>,
            ),
            (
                Arc::new(Field::new(
                    "multiple",
                    list_array.data_type().clone(),
                    false,
                )),
                Arc::new(list_array) as Arc<dyn arrow::array::Array>,
            ),
            (
                Arc::new(Field::new("stairstep", DataType::Int32, false)),
                Arc::new(Int32Array::from(vec![stairstep_value])) as Arc<dyn arrow::array::Array>,
            ),
            (
                Arc::new(Field::new("toggle", DataType::Boolean, false)),
                Arc::new(BooleanArray::from(vec![toggle_value])) as Arc<dyn arrow::array::Array>,
            ),
            (
                Arc::new(Field::new(
                    "properties",
                    properties_struct.data_type().clone(),
                    false,
                )),
                Arc::new(properties_struct) as Arc<dyn arrow::array::Array>,
            ),
        ]);

        test_context.log_entity("plots/sigmoids", |builder| {
            builder.with_archetype_auto_row(
                timepoint,
                &DynamicArchetype::new("custom")
                    .with_component_from_data("data", Arc::new(struct_array)),
            )
        });
    }
}

/// Builtin enum components (like `MarkerShape`, which is `UInt8`) should NOT be visualizable by the
/// time series view, even when the blueprint explicitly maps them to the scalar slot.
///
/// `MarkerShape` has a `UInt8` physical representation which overlaps with scalar-compatible types,
/// but it's a known enum and has no meaningful numeric interpretation for plotting.
#[test]
pub fn test_builtin_enum_not_visualizable_as_scalar() {
    let mut test_context = TestContext::new();
    test_context.register_view_class::<TimeSeriesView>();

    let timeline = test_context.active_timeline().unwrap();

    // Log only a MarkerShape component (`UInt8` enum) on a temporal timeline.
    for i in 0..5 {
        test_context.log_entity("plots/markers_only", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, i)],
                &DynamicArchetype::new("custom").with_component::<components::MarkerShape>(
                    "markers",
                    [components::MarkerShape::Circle],
                ),
            )
        });
    }

    // Explicitly set up a SeriesLines visualizer that maps the MarkerShape component to scalar.
    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        use re_sdk_types::blueprint::encodings::{ComponentSourceKind, VisualizerComponentMapping};

        let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());

        ctx.save_visualizers(
            &EntityPath::from("plots/markers_only"),
            view.id,
            [SeriesLines::new().visualizer().with_mappings([
                blueprint::components::VisualizerComponentMapping(VisualizerComponentMapping {
                    target: Scalars::descriptor_scalars().component.as_str().into(),
                    source_kind: ComponentSourceKind::SourceComponent,
                    source_component: Some("custom:markers".into()),
                    selector: None,
                }),
            ])],
        );

        blueprint.add_view_at_root(view)
    });

    let query_result = test_context
        .query_results
        .get(&view_id)
        .expect("View should have query results");
    let data_result_tree = &query_result.tree;

    // The entity should not be visualizable at all, because MarkerShape is a known
    // builtin enum and should be filtered out at the subscriber level despite being UInt8.
    let result =
        data_result_tree.lookup_result_by_path(EntityPath::from("plots/markers_only").hash());

    assert!(
        result.is_none_or(|r| r.visualizer_instructions.is_empty()),
        "Entity with only a builtin enum component (MarkerShape/UInt8) should not have any \
         active visualizers, even when explicitly configured in the blueprint, but got: {result:?}",
    );
}

fn setup_blueprint_with_explicit_mapping_nested(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        use re_sdk_types::blueprint::encodings::{ComponentSourceKind, VisualizerComponentMapping};

        let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());

        // Multiple visualizers for the `plots/sigmoids` entity:
        // * Points (for .single field):
        //    * scalar - map to data.single
        //    * marker - use Circle markers
        //    * color - map to data.properties.color[]
        //    * name - map to data.properties.name
        // * Lines (for .multiple[] field elements):
        //    * scalar - map to data.multiple[]
        //    * no explicit styling (auto)
        // * Lines (for .stairstep field):
        //    * scalar - map to data.stairstep (Int32, tests casting)
        //    * explicit red color, staircase from -2 to 2
        // * Lines (for .toggle field):
        //    * scalar - map to data.toggle (Boolean, tests bool-to-scalar casting)
        //    * explicit green color, square wave toggling every 4 ticks
        let single_mapping = VisualizerComponentMapping {
            target: Scalars::descriptor_scalars().component.as_str().into(),
            source_kind: ComponentSourceKind::SourceComponent,
            source_component: Some("custom:data".into()),
            selector: Some(".single".into()),
        };

        let color_mapping = VisualizerComponentMapping {
            target: SeriesPoints::descriptor_colors().component.as_str().into(),
            source_kind: ComponentSourceKind::SourceComponent,
            source_component: Some("custom:data".into()),
            selector: Some(".properties.color[]".into()),
        };

        let name_mapping = VisualizerComponentMapping {
            target: SeriesPoints::descriptor_names().component.as_str().into(),
            source_kind: ComponentSourceKind::SourceComponent,
            source_component: Some("custom:data".into()),
            selector: Some(".properties.name".into()),
        };

        let multiple_mapping = VisualizerComponentMapping {
            target: Scalars::descriptor_scalars().component.as_str().into(),
            source_kind: ComponentSourceKind::SourceComponent,
            source_component: Some("custom:data".into()),
            selector: Some(".multiple[]".into()),
        };

        let stairstep_mapping = VisualizerComponentMapping {
            target: Scalars::descriptor_scalars().component.as_str().into(),
            source_kind: ComponentSourceKind::SourceComponent,
            source_component: Some("custom:data".into()),
            selector: Some(".stairstep".into()),
        };

        let toggle_mapping = VisualizerComponentMapping {
            target: Scalars::descriptor_scalars().component.as_str().into(),
            source_kind: ComponentSourceKind::SourceComponent,
            source_component: Some("custom:data".into()),
            selector: Some(".toggle".into()),
        };

        let visualizers = vec![
            SeriesPoints::new()
                .with_markers([components::MarkerShape::Circle])
                .visualizer()
                .with_mappings([
                    blueprint::components::VisualizerComponentMapping(single_mapping),
                    blueprint::components::VisualizerComponentMapping(color_mapping),
                    blueprint::components::VisualizerComponentMapping(name_mapping),
                ]),
            SeriesLines::new().visualizer().with_mappings([
                blueprint::components::VisualizerComponentMapping(multiple_mapping),
            ]),
            SeriesLines::new()
                .with_colors([components::Color::from_rgb(255, 0, 0)])
                .visualizer()
                .with_mappings([blueprint::components::VisualizerComponentMapping(
                    stairstep_mapping,
                )]),
            SeriesLines::new()
                .with_colors([components::Color::from_rgb(0, 128, 0)])
                .visualizer()
                .with_mappings([blueprint::components::VisualizerComponentMapping(
                    toggle_mapping,
                )]),
        ];

        ctx.save_visualizers(&EntityPath::from("plots/sigmoids"), view.id, visualizers);

        blueprint.add_view_at_root(view)
    })
}

#[test]
pub fn test_transform3d_time_series() {
    let mut test_context = TestContext::new();
    test_context.register_view_class::<TimeSeriesView>();

    let timeline = test_context.active_timeline().unwrap();
    log_data_transform3d(&mut test_context, timeline);

    let view_id = setup_blueprint_transform3d(&mut test_context);

    let size = egui::vec2(300.0, 300.0);
    let mut snapshot_results = SnapshotResults::new();
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "transform3d_time_series",
        size,
        None,
    ));
}

fn log_data_transform3d(test_context: &mut TestContext, timeline: re_log_types::Timeline) {
    for i in 0..=MAX_TIME {
        let timepoint = TimePoint::from([(timeline, i)]);
        let t = i as f64 / 8.0;

        // Phase offsets ensure no translation component is ever exactly zero.
        let translation = [
            (t + 0.2).sin() as f32,
            (t * 0.7 + 0.4).sin() as f32,
            (t * 0.5 + 0.6).sin() as f32,
        ];

        // Rotate around a general axis (1, 2, 3) with an initial angle offset
        // so all four quaternion components are always non-zero.
        let angle = t as f32 + 0.5;
        let half = angle / 2.0;
        let norm = (1.0_f32 + 4.0 + 9.0).sqrt();
        let (ax, ay, az) = (1.0 / norm, 2.0 / norm, 3.0 / norm);

        test_context.log_entity("plots/transform", |builder| {
            builder.with_archetype_auto_row(
                timepoint,
                &Transform3D::from_translation_rotation(
                    translation,
                    encodings::Quaternion::from_xyzw([
                        ax * half.sin(),
                        ay * half.sin(),
                        az * half.sin(),
                        half.cos(),
                    ]),
                ),
            )
        });
    }
}

fn setup_blueprint_transform3d(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        use re_sdk_types::blueprint::encodings::{ComponentSourceKind, VisualizerComponentMapping};

        let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());

        let translation_source = Transform3D::descriptor_translation().component.as_str();
        let quaternion_source = Transform3D::descriptor_quaternion().component.as_str();

        ctx.save_visualizers(
            &EntityPath::from("plots/transform"),
            view.id,
            [
                SeriesLines::new().visualizer().with_mappings([
                    blueprint::components::VisualizerComponentMapping(VisualizerComponentMapping {
                        target: Scalars::descriptor_scalars().component.as_str().into(),
                        source_kind: ComponentSourceKind::SourceComponent,
                        source_component: Some(translation_source.into()),
                        // TODO(RR-3435): Once we can index into `FixedSizeListArray` using selectors,
                        // we should unroll the translation components via `[0]` to test that too.
                        selector: Some("[]".into()),
                    }),
                ]),
                SeriesLines::new().visualizer().with_mappings([
                    blueprint::components::VisualizerComponentMapping(VisualizerComponentMapping {
                        target: Scalars::descriptor_scalars().component.as_str().into(),
                        source_kind: ComponentSourceKind::SourceComponent,
                        source_component: Some(quaternion_source.into()),
                        selector: Some("[]".into()),
                    }),
                ]),
            ],
        );

        blueprint.add_view_at_root(view)
    })
}
