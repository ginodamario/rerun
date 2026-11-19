// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Identifies one instance lane in a State Timeline view.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, PartialEq, Eq))]
#[rerun(state = "unstable")]
pub struct StateLaneSource {
    /// ID of the State Timeline view.
    pub view_id: rerun::encodings::Uuid,

    /// Entity that provides the states.
    pub entity_path: rerun::encodings::EntityPath,

    /// ID of the state visualizer instruction for this entity.
    pub instruction_id: rerun::encodings::Uuid,

    /// Zero-based index into the logged state array.
    pub instance: u64,
}
