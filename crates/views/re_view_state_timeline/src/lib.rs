//! Rerun state timeline View.
//!
//! A View that shows state transitions as horizontal lanes over time.

mod background;
mod data;
mod view_class;
mod visualizer;
mod visualizer_ui;

pub use background::time_series_background;
pub use data::{
    StateLane, StateLaneGroup, StateLanePhase, StateLanePhaseContent, StateLanesOutput,
    StateValueKind,
};
pub use view_class::{StateTimelineView, TimeViewProperty};
pub use visualizer::StateVisualizer;
