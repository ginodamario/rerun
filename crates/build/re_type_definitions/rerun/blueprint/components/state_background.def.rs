// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The state lane used to color the backgrounds of time series views.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, PartialEq, Eq))]
#[rerun(state = "unstable")]
pub struct StateBackground {
    pub source: rerun::blueprint::encodings::StateLaneSource,
}
