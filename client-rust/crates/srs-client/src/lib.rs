//! Headless client engine. Everything the C# `MainWindowViewModel`,
//! `AudioManager`, `UDPClientAudioProcessor` and `DCSRadioSyncHandler` do,
//! minus the UI. [`Client`] is the façade the UI drives.

pub mod client;
pub mod combined_state;
pub mod dcs_io;
pub mod dcs_sync;
pub mod decode;
pub mod eam;
pub mod encoder;
pub mod ptt;
pub mod radio_state;
pub mod state;
pub mod tx;

pub use client::{build_client, AppEvent, Client, WakeFn};
pub use dcs_io::{DcsEvent, DcsPorts};
pub use encoder::TxControls;
pub use ptt::{PttHandler, PttSettings};
pub use radio_state::{RadioState, ReceivingState, SendingState, SharedRadioState, SideInfo};
pub use srs_dcs::merge::MergeContext;
pub use state::ClientState;
