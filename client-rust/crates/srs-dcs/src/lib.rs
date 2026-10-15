//! DCS side of the client: the radio state as exported by the Lua scripts
//! (`DCSPlayerRadioInfo`, `DCSRadio`) and, later, the local UDP listeners
//! and the merge rules of `DCSRadioSyncHandler`.

pub mod merge;
pub mod model;
pub mod presets;
pub mod radio_helper;
pub mod transponder;

pub use merge::{merge, MergeContext, MergeEvent, MergeMemory, MergeOutcome};
pub use model::{
    DcsAircraftCapabilities, DcsPlayerRadioInfo, DcsRadio, EncryptionMode, FreqMode, RetransmitMode,
    SimultaneousTransmissionControl, SwitchControls, VolumeMode, UNIT_ID_OFFSET,
};
