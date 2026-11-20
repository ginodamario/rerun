use re_sdk_types::components;
use re_sdk_types::{Archetype as _, archetypes};
use re_viewer_context::{
    IdentifiedViewSystem, SingleRequiredComponentConstraint, ViewContext, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};

use crate::line_series_loader::{
    LineSeriesSource, LineSeriesStyling, load_line_series_with_styling,
};
use crate::line_visualizer_system::build_line_draw_data;
use crate::util;

/// The system for rendering [`archetypes::Measurements`] archetypes.
///
/// Draws each series as a line with an error band one standard deviation wide around it.
#[derive(Default, Debug)]
pub struct MeasurementsSeriesSystem;

impl IdentifiedViewSystem for MeasurementsSeriesSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "MeasurementsSeries"
        )
    }
}

impl VisualizerSystem for MeasurementsSeriesSystem {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo {
            relevant_archetype: archetypes::Measurements::name().into(),
            annotation_context: None,
            constraints: SingleRequiredComponentConstraint::new::<components::Scalar>(
                &archetypes::Measurements::descriptor_values(),
            )
            .with_additional_physical_types(util::series_supported_encodings())
            .with_allow_static_data(false)
            .into(),

            queried: archetypes::Measurements::all_components()
                .iter()
                .cloned()
                .collect(),
        }
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context: &re_viewer_context::ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let output = VisualizerExecutionOutput::default();

        let source = LineSeriesSource {
            value_descriptor: archetypes::Measurements::descriptor_values(),
            queried_components: archetypes::Measurements::all_component_identifiers().collect(),
            styling: LineSeriesStyling::measurements(),
            variance_descriptor: Some(archetypes::Measurements::descriptor_variances()),
            unit_descriptor: Some(archetypes::Measurements::descriptor_units()),
        };

        let series = crate::plot_query::load_plot_series(
            ctx,
            query,
            Self::identifier(),
            &output,
            |data, instruction, window| {
                load_line_series_with_styling(
                    ctx,
                    query,
                    window,
                    data,
                    instruction,
                    &output,
                    &source,
                )
            },
        );
        let draw_data = build_line_draw_data(ctx, query, &series.all_series)?;

        Ok(output
            .with_draw_data(draw_data)
            .with_visualizer_data(series))
    }
}
