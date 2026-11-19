use crate::legend::LegendSwatch;
use egui::{Atom, AtomExt as _, TextWrapMode, WidgetAtom};
use re_ui::{DesignTokens, UiExt as _};

/// Shows a plot tooltip at the pointer for the given response.
///
/// The tooltip has a small header label, followed by a value line with a color swatch,
/// series label, and right-aligned value text.
pub fn show_plot_tooltip(
    ui: &egui::Ui,
    response: &egui::Response,
    id_salt: impl egui::AsIdSalt,
    header: &str,
    label: &str,
    value: &str,
    color: egui::Color32,
) {
    show_plot_tooltip_rows(ui, response, id_salt, header, [(label, value, color)], 0);
}

/// Shows a plot tooltip with a header, one value row per series, and an omitted-row count.
pub fn show_plot_tooltip_rows<'a>(
    ui: &egui::Ui,
    response: &egui::Response,
    id_salt: impl egui::AsIdSalt,
    header: &str,
    rows: impl IntoIterator<Item = (&'a str, &'a str, egui::Color32)>,
    omitted_rows: usize,
) {
    PlotTooltipStyle::Labeled.show_at_pointer(ui, response, id_salt, header, rows, omitted_rows);
}

/// Appearance of plot value readouts.
#[derive(Clone, Copy)]
pub enum PlotTooltipStyle {
    /// Framed popup with a swatch, series name, and value.
    Labeled,

    /// Transparent overlay with a swatch and value.
    Compact,
}

impl PlotTooltipStyle {
    /// Shows the readout next to the pointer using the plot tooltip delay.
    pub fn show_at_pointer<'a>(
        self,
        ui: &egui::Ui,
        response: &egui::Response,
        id_salt: impl egui::AsIdSalt,
        header: &str,
        rows: impl IntoIterator<Item = (&'a str, &'a str, egui::Color32)>,
        omitted_rows: usize,
    ) {
        let mut tooltip = plot_tooltip(ui, response, id_salt);
        if matches!(self, Self::Compact) {
            tooltip.popup = tooltip.popup.frame(egui::Frame::NONE);
        }
        tooltip.show(|ui| self.show_rows(ui, header, rows, omitted_rows));
    }

    /// Draws the time or index header, series values, and omitted-row count.
    pub fn show_rows<'a>(
        self,
        ui: &mut egui::Ui,
        header: &str,
        rows: impl IntoIterator<Item = (&'a str, &'a str, egui::Color32)>,
        omitted_rows: usize,
    ) {
        let tokens = ui.tokens();

        ui.add(
            egui::Label::new(
                egui::RichText::new(header)
                    .size(DesignTokens::combo_item_small_font_size())
                    .color(tokens.text_default),
            )
            .truncate(),
        );

        ui.add_space(tokens.text_to_icon_padding());

        for (label, value, color) in rows {
            self.value_row(ui, label, value, color);
        }

        if omitted_rows > 0 {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!("… and {omitted_rows} more"))
                        .color(tokens.list_item_noninteractive_text),
                )
                .truncate(),
            );
        }
    }

    fn value_row(
        self,
        ui: &mut egui::Ui,
        label_text: &str,
        value_text: &str,
        color: egui::Color32,
    ) {
        const MAX_WIDTH: f32 = 500.0;
        const SWATCH_GAP: f32 = 6.0;
        const LABEL_VALUE_GAP: f32 = 16.0;

        let tokens = ui.tokens();

        let swatch = LegendSwatch {
            color,
            visible: true,
        }
        .atom();
        let value = egui::RichText::new(value_text).color(tokens.list_item_strong_text);
        let atoms = match self {
            Self::Labeled => {
                let label = egui::RichText::new(label_text)
                    .color(tokens.list_item_noninteractive_text)
                    .atom_shrink(true);
                let spacer =
                    Atom::default().atom_size(egui::vec2(LABEL_VALUE_GAP - SWATCH_GAP * 2.0, 0.0));
                egui::Atoms::new((swatch, label, spacer, value))
            }
            Self::Compact => egui::Atoms::new((swatch, value)),
        };

        let width = ui.available_width().min(MAX_WIDTH);
        ui.set_max_width(width);

        let atom_layout = WidgetAtom::new(atoms)
            .gap(SWATCH_GAP)
            .max_width(width)
            .sense(egui::Sense::hover())
            .wrap_mode(TextWrapMode::Truncate)
            .allocate(ui);

        atom_layout.response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Label,
                ui.is_enabled(),
                format!("{label_text} {value_text}"),
            )
        });
        atom_layout.paint(ui);
    }
}

/// Creates a plot tooltip for the given response with Rerun's plot tooltip settings.
///
/// Temporarily overrides the global egui style to use a shorter tooltip delay (150ms)
/// and keeps the tooltip visible once shown. Restores the original style after creation.
fn plot_tooltip<'a>(
    ui: &egui::Ui,
    response: &'a egui::Response,
    id_salt: impl egui::AsIdSalt,
) -> egui::Tooltip<'a> {
    let prev_style = ui.ctx().global_style();
    ui.ctx().global_style_mut(|style| {
        style.interaction.tooltip_delay = 0.15;
        style.interaction.tooltip_grace_time = 3.0;
    });

    let is_open = response.enabled() && egui::Tooltip::should_show_tooltip(response, true);
    let mut tooltip = egui::Tooltip::for_widget(response);
    tooltip.popup = tooltip.popup.open(is_open).id(response.id.with(id_salt));

    ui.ctx().set_global_style(prev_style);

    tooltip
        .at_pointer()
        .gap(f32::from(ui.tokens().view_padding()))
}

/// Renders a single value line in the plot tooltip with truncation.
///
/// Max total width is 500px. The label is marked as `shrink` so it truncates
/// first when space is tight, while the value stays fully visible.
pub fn plot_tooltip_label_value(
    ui: &mut egui::Ui,
    label_text: &str,
    value_text: &str,
    color: egui::Color32,
) {
    PlotTooltipStyle::Labeled.value_row(ui, label_text, value_text, color);
}
