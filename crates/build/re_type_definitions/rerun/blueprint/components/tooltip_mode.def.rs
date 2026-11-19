// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Controls where the plot shows series values.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub enum TooltipMode {
    /// Show only the nearest data point to the mouse pointer.
    Nearest = 1,

    /// Show values of all visible series at the hovered time.
    All = 2,

    /// Show values of visible series beside the Viewer time cursor.
    #[default]
    TimeCursor = 3,
}
