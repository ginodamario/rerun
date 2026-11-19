//! The state lane selected in the blueprint to shade all time series views.

use std::sync::Arc;

use re_chunk_store::MissingChunkReporter;
use re_log_types::{AbsoluteTimeRange, EntityPath};
use re_sdk_types::blueprint::archetypes::ViewportBlueprint;
use re_sdk_types::blueprint::components::StateBackground;
use re_sdk_types::blueprint::encodings::StateLaneSource;
use re_viewer_context::{
    BlueprintContext as _, DataResult, IdentifiedViewSystem as _, ViewContext, ViewQuery,
    ViewerContext, VisualizerExecutionOutput, VisualizerInstruction,
};
use re_viewport_blueprint::{VIEWPORT_PATH, ViewBlueprint};

use crate::StateVisualizer;
use crate::visualizer::{LaneGroupBuilder, visible_time_range};

fn selected_source(ctx: &ViewerContext<'_>) -> Option<StateLaneSource> {
    let (_, selection) = ctx
        .blueprint_db()
        .latest_at_component_quiet::<StateBackground>(
            &VIEWPORT_PATH.into(),
            ctx.blueprint_query,
            ViewportBlueprint::descriptor_state_background().component,
        )?;
    Some(selection.0)
}

pub(crate) fn background_selection_ui(
    ui: &mut egui::Ui,
    ctx: &ViewContext<'_>,
    data_result: &DataResult,
    instruction: &VisualizerInstruction,
    instance_count: usize,
) {
    let mut source = StateLaneSource {
        view_id: ctx.view_id.into(),
        entity_path: (&data_result.entity_path).into(),
        instruction_id: instruction.id.0,
        instance: 0,
    };
    let selected = selected_source(ctx.viewer_ctx).filter(|selected| {
        selected.view_id == source.view_id
            && selected.entity_path == source.entity_path
            && selected.instruction_id == source.instruction_id
    });
    let mut enabled = selected.is_some();
    if let Some(selected) = selected {
        source.instance = selected.instance;
    }
    let mut changed = ui.checkbox(&mut enabled, "Show as time series background")
        .on_hover_text("Use this lane’s state colors behind every time series plot. Selecting another lane replaces this one.")
        .changed();
    if enabled && (instance_count > 1 || source.instance > 0) {
        ui.horizontal(|ui| {
            ui.label("State instance");
            let max_index = source.instance.max(instance_count.saturating_sub(1) as u64);
            changed |= ui
                .add(egui::DragValue::new(&mut source.instance).range(0..=max_index))
                .changed();
        });
    }
    if changed {
        ctx.viewer_ctx.save_blueprint_component(
            VIEWPORT_PATH.into(),
            &ViewportBlueprint::descriptor_state_background(),
            &enabled.then_some(StateBackground(source)),
        );
    }
}

type ColoredRange = (AbsoluteTimeRange, egui::Color32);

type BackgroundCache = egui::cache::FramePublisher<u64, Arc<[ColoredRange]>>;

/// Query the pinned lane in the time series plot's window, including the state already
/// active at the left edge. The source view can be in a different tab or zoomed elsewhere.
pub fn time_series_background(
    ctx: &ViewerContext<'_>,
    query: &ViewQuery<'_>,
    plot_window: AbsoluteTimeRange,
    missing_chunks: &MissingChunkReporter,
) -> Option<Arc<[ColoredRange]>> {
    let selection = selected_source(ctx)?;
    let view_id = selection.view_id.into();
    let result = ctx.query_results.get(&view_id)?;
    let path = EntityPath::from(selection.entity_path);
    let data_result = result.tree.lookup_result_by_path(path.hash())?;
    if !data_result.is_visible() {
        return None;
    }
    let instruction = data_result
        .visualizer_instructions
        .iter()
        .find(|instruction| {
            instruction.id.0 == selection.instruction_id
                && instruction.visualizer_type == StateVisualizer::identifier()
        })?;

    let clip = plot_window
        .intersection(ctx.recording().time_range_for(&query.timeline)?)?
        .intersection(visible_time_range(ctx, data_result))?;
    // Plots with linked axes commonly request exactly the same ranges. Share the decoded
    // phases within this paint pass, then query afresh for streamed data and blueprint edits.
    let egui_ctx = ctx.egui_ctx();
    let key = egui::util::hash((
        egui_ctx.cumulative_pass_nr(),
        ctx.store_id(),
        ctx.blueprint_db().store_id(),
        ctx.blueprint_query,
        view_id,
        path.hash(),
        instruction.id,
        selection.instance,
        query.timeline,
        query.latest_at,
        clip,
    ));
    if let Some(cached) =
        egui_ctx.memory_mut(|mem| mem.caches.cache::<BackgroundCache>().get(&key).cloned())
    {
        return Some(cached);
    }
    let view = ViewBlueprint::try_from_db(view_id, ctx.blueprint_db(), ctx.blueprint_query)?;
    let view_ctx = view.bundle_context_with_state(ctx, &());
    let output = VisualizerExecutionOutput::default();
    let builder = LaneGroupBuilder::new(&view_ctx, query.timeline, query.latest_at, &output);
    let source = builder.lane_source(data_result, instruction, Some(clip));
    if output.any_missing_chunks() {
        missing_chunks.report_missing_chunk();
    }
    let lane = source?.lane(usize::try_from(selection.instance).ok()?);
    let mut bands = Vec::new();
    let clip_end = clip.max().as_i64();
    for (i, phase) in lane.phases.iter().enumerate() {
        if let Some(content) = &phase.content {
            let start = phase.start_time.max(clip.min().as_i64());
            let end = lane
                .phases
                .get(i + 1)
                .map_or(clip_end, |next| next.start_time.min(clip_end));
            if start < end {
                bands.push((
                    AbsoluteTimeRange::new(start, end),
                    background_color(content.color),
                ));
            }
        }
    }
    let ranges: Arc<[_]> = bands.into();
    if !output.any_missing_chunks() {
        egui_ctx.memory_mut(|mem| {
            mem.caches
                .cache::<BackgroundCache>()
                .set(key, ranges.clone());
        });
    }
    Some(ranges)
}

/// Use the same translucent tint for pinned and hovered state phases.
pub(crate) fn background_color(color: egui::Color32) -> egui::Color32 {
    color.gamma_multiply_u8(30)
}
