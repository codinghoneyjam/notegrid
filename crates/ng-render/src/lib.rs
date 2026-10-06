// <META - FILE SUMMARY - ng-render: Renderer<S: Synth> FSM + render_offline; ng-dsp master bus. | L1-6>
// HOTPATH crate (SET HOTPATH in spec/dsl/00_dsl.dsl): the workspace deny list applies here.
// R05.R05: this crate names only the Synth trait, never OscSynth/SoundFontSynth.
pub mod master;
pub mod mix;
pub mod offline;
pub mod renderer;

pub use master::MasterBus;
pub use mix::{effective_gain, LaneMix};
pub use offline::{render_offline, LoopMode, OfflineOptions, RenderError};
pub use renderer::{BlockStatus, RenderConfig, Renderer, TransportState};
