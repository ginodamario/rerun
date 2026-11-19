use re_log_types::{EntityPath, TimePoint, Timeline};
use re_sdk_types::archetypes::{Scalars, StateChange, StateConfiguration};
use re_sdk_types::blueprint::archetypes::TimeAxis;
use re_sdk_types::blueprint::components::LinkAxis;
use re_sdk_types::components::Color;
use re_sdk_types::encodings::{TimeRange, TimeRangeBoundary};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::{Harness, kittest::Queryable as _};
use re_test_viewport::TestContextExt as _;
use re_ui::UiExt as _;
use re_view_state_timeline::{StateTimelineView, StateVisualizer};
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{BlueprintContext as _, ViewClass as _, VisualizerSystem as _};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

#[test]
fn pin_state_lane_from_inspector() {
    let mut ctx = TestContext::new_with_view_class::<TimeSeriesView>();
    ctx.register_view_class::<StateTimelineView>();
    let timeline = Timeline::log_tick();
    for (time, state) in [(0, "Idle"), (10, "Hidden"), (20, "Running"), (30, "Idle")] {
        ctx.log_entity("mode", |builder| {
            builder.with_archetype_auto_row([(timeline, time)], &StateChange::single(state))
        });
    }
    ctx.log_entity("mode", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &StateConfiguration::new()
                .with_values(["Idle", "Hidden", "Running"])
                .with_colors([
                    Color::from_rgb(255, 0, 0),
                    Color::from_rgb(0, 0, 255),
                    Color::from_rgb(0, 255, 0),
                ])
                .with_visible([true, false, true]),
        )
    });
    for time in [0, 34] {
        ctx.log_entity("speed", |builder| {
            builder.with_archetype_auto_row([(timeline, time)], &Scalars::single(time as f64))
        });
    }
    ctx.set_active_timeline(*timeline.name());
    let (state_id, plot_id) = ctx.setup_viewport_blueprint(|ctx, blueprint| {
        let state = ViewBlueprint::new_with_root_wildcard(StateTimelineView::identifier());
        let plot = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());
        let plot_axis = ViewProperty::from_archetype_for_view::<TimeAxis>(ctx, plot.id);
        ctx.save_blueprint_archetype(
            plot_axis.blueprint_store_path,
            &TimeAxis::new()
                .with_link(LinkAxis::Independent)
                .with_view_range(TimeRange {
                    start: TimeRangeBoundary::Absolute(5.into()),
                    end: TimeRangeBoundary::Absolute(40.into()),
                }),
        );
        (
            blueprint.add_view_at_root(state),
            blueprint.add_view_at_root(plot),
        )
    });

    let show_inspector = std::cell::Cell::new(true);
    let mut harness = ctx
        .setup_kittest_for_rendering_ui(egui::vec2(500.0, 300.0))
        .build_ui(|ui| {
            if show_inspector.get() {
                ctx.run_ui(ui, |ctx, ui| {
                    let view = ViewBlueprint::try_from_db(
                        state_id,
                        ctx.blueprint_db(),
                        ctx.blueprint_query,
                    )
                    .expect("state view");
                    let source = view.bundle_context_with_state(ctx, &());
                    let data = source
                        .query_result
                        .tree
                        .lookup_result_by_path(EntityPath::from("mode").hash())
                        .expect("mode entity");
                    ui.list_item_scope("inspector", |ui| {
                        StateVisualizer.selection_ui(
                            &source,
                            ui,
                            data,
                            &data.visualizer_instructions[0],
                            None,
                        )
                    });
                });
                ctx.handle_system_commands(ui.ctx());
            } else {
                ctx.run_with_single_view(ui, plot_id);
            }
        });
    let label = "Show as time series background";
    harness.get_by_label(label).click();
    harness.run();
    show_inspector.set(false);
    harness.remove_cursor();
    harness.run();

    let red = egui::Color32::RED.gamma_multiply_u8(30);
    let green = egui::Color32::GREEN.gamma_multiply_u8(30);
    let visible_bands = |harness: &Harness<'_>| {
        harness
            .output()
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect) if [red, green].contains(&rect.fill) => {
                    Some((rect.fill, rect.rect))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let bands = visible_bands(&harness);
    assert_eq!(
        bands.iter().map(|(color, _)| *color).collect::<Vec<_>>(),
        [red, green, red]
    );
    let [(_, first), (_, second), (_, last)] = bands[..] else {
        unreachable!()
    };
    // [5,10], [20,30], [30,34]: includes a bootstrapped state, a hidden interval,
    // and a final phase clipped at the end of the recording.
    let unit = second.width() / 10.0;
    assert!((first.width() - 5.0 * unit).abs() < 0.1);
    assert!((second.left() - first.right() - 10.0 * unit).abs() < 0.1);
    assert!((last.width() - 4.0 * unit).abs() < 0.1);
    assert_eq!(second.right(), last.left());

    show_inspector.set(true);
    harness.run();
    harness.get_by_label(label).click();
    harness.run();
    show_inspector.set(false);
    harness.remove_cursor();
    harness.run();
    assert!(visible_bands(&harness).is_empty());
}
