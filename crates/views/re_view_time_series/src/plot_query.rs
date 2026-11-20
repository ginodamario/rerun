//! Load the visible plot window and, when pinned, sample the two cursor positions.

use std::collections::BTreeMap;

use rayon::prelude::*;
use re_log_types::{AbsoluteTimeRange, TimeInt};
use re_viewer_context::{
    DataResult, ViewContext, ViewQuery, ViewStateExt as _, ViewSystemIdentifier,
    ViewerReportSeverity, VisualizerExecutionOutput, VisualizerInstruction,
};

use crate::view_class::{TimeSeriesViewState, series_value_at_time};
use crate::{PlotSeries, PlotSeriesKind, util};

#[derive(Clone, Copy)]
pub(crate) struct PlotWindow {
    pub range: AbsoluteTimeRange,
    pub time_per_pixel: f64,
}

#[derive(Default, Clone)]
pub(crate) struct TimeSeriesOutput {
    pub all_series: Vec<PlotSeries>,
    pub readings: Vec<CursorReading>,
}

#[derive(Clone)]
pub(crate) struct CursorReading {
    pub label: String,
    pub color: egui::Color32,
    pub unit: Option<String>,
    pub value: Option<f64>,
    pub reference_value: Option<f64>,
}

pub(crate) fn load_plot_series(
    ctx: &ViewContext<'_>,
    query: &ViewQuery<'_>,
    visualizer: ViewSystemIdentifier,
    output: &VisualizerExecutionOutput,
    load: impl Fn(&DataResult, &VisualizerInstruction, PlotWindow) -> Vec<PlotSeries> + Sync,
) -> TimeSeriesOutput {
    let time_per_pixel = ctx
        .view_state
        .downcast_ref::<TimeSeriesViewState>()
        .map_or(1.0, |state| state.time_per_pixel);
    let reference = ctx.viewer_ctx.time_ctrl.reference_time().filter(|_| {
        ctx.viewer_ctx.app_options().show_time_series_tooltips
            || util::tooltip_key_held(ctx.egui_ctx())
    });

    let instructions: Vec<_> = query.iter_visualizer_instruction_for(visualizer).collect();
    let results: Vec<_> = instructions
        .par_iter()
        .map(|&(data, instruction)| {
            let data_range = util::data_result_time_range(ctx.viewer_ctx, data, query.timeline);
            let range = match util::determine_query_range(ctx, data_range) {
                Ok(range) => range,
                Err(err) => {
                    output.report_unspecified_source(
                        instruction.id,
                        ViewerReportSeverity::Error,
                        format!("Failed to determine query range: {err}"),
                    );
                    return TimeSeriesOutput::default();
                }
            };
            let all_series = load(
                data,
                instruction,
                PlotWindow {
                    range,
                    time_per_pixel,
                },
            );
            let readings = reference.map_or_else(Vec::new, |reference| {
                let sample = |time| {
                    load(
                        data,
                        instruction,
                        PlotWindow {
                            range: AbsoluteTimeRange::point(time),
                            // Preserve individual timestamps even when the plot is zoomed far out.
                            time_per_pixel: 1.0,
                        },
                    )
                };
                compare_cursors(&all_series, data_range, query.latest_at, reference, sample)
            });
            TimeSeriesOutput {
                all_series,
                readings,
            }
        })
        .collect();
    let mut output = TimeSeriesOutput::default();
    for result in results {
        output.all_series.extend(result.all_series);
        output.readings.extend(result.readings);
    }
    output
}

/// Runs split by styling or clears still belong to the same signal instance.
fn compare_cursors(
    plotted: &[PlotSeries],
    data_range: AbsoluteTimeRange,
    playback: TimeInt,
    reference: TimeInt,
    sample: impl Fn(TimeInt) -> Vec<PlotSeries>,
) -> Vec<CursorReading> {
    let mut rows = BTreeMap::new();
    let mut collect = |runs: &[PlotSeries], cursor: Option<TimeInt>, is_reference: bool| {
        for run in runs {
            if !run.visible || run.kind == PlotSeriesKind::Clear {
                continue;
            }
            let instance = run
                .instance_path
                .instance
                .specific_index()
                .map_or(0, |i| i.get());
            let row = rows.entry(instance).or_insert_with(|| CursorReading {
                color: run.color,
                label: run.label.clone(),
                unit: run.unit.clone(),
                value: None,
                reference_value: None,
            });
            if let Some(value) =
                cursor.and_then(|time| series_value_at_time(&run.points, run.kind, time.as_i64()))
            {
                if is_reference {
                    if row.unit == run.unit {
                        row.reference_value = Some(value);
                    }
                } else {
                    row.value = Some(value);
                    row.color = run.color;
                    row.label.clone_from(&run.label);
                    row.unit.clone_from(&run.unit);
                }
            }
        }
    };
    collect(plotted, None, false);
    for (time, is_reference) in [(playback, false), (reference, true)] {
        if !data_range.contains(time) || (is_reference && playback == reference) {
            continue;
        }
        // Sample at a fixed resolution so zooming or panning cannot change a measurement.
        // Line queries include the neighboring points needed for interpolation.
        collect(&sample(time), Some(time), is_reference);
    }
    if playback == reference {
        for row in rows.values_mut() {
            row.reference_value = row.value;
        }
    }
    rows.into_values().collect()
}
