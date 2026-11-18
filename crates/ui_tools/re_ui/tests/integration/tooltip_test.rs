use egui_kittest::{Harness, kittest::Queryable as _};

#[test]
fn moving_between_widgets_restarts_tooltip_delay() {
    let mut harness = Harness::builder()
        .with_context_setup(re_ui::apply_style_and_install_loaders)
        .with_step_dt(0.05)
        .build_ui(|ui| {
            ui.horizontal(|ui| {
                ui.button("First").on_hover_text("First tooltip");
                ui.button("Second").on_hover_text("Second tooltip");
            });
        });

    harness.get_by_label("First").hover();
    harness.run_steps(12);
    harness.get_by_label("First tooltip");

    harness.get_by_label("Second").hover();
    harness.run_steps(3);
    assert!(
        harness.query_by_label("First tooltip").is_none(),
        "the previous tooltip should close when the pointer moves to another widget"
    );
    assert!(
        harness.query_by_label("Second tooltip").is_none(),
        "the next tooltip should wait for the hover delay"
    );

    harness.run_steps(12);
    harness.get_by_label("Second tooltip");
}
