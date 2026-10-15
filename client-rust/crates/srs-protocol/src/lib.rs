//! SRS wire protocol, shared by every other crate of the Rust client.
//!
//! This crate is pure: no sockets, no threads, no clocks. Everything in it is
//! byte-compatible with the C# implementation in `Common/` of the upstream
//! repository (see `docs-agents/architecture/protocole-reseau.md`).
//!
//! Rules:
//! - Never reorder or rename anything that reaches the wire (`MessageType`,
//!   `ServerSettingsKey`, JSON field names, the UDP layout).
//! - Behaviour quirks of the C# code are reproduced on purpose and documented
//!   where they occur; do not "fix" them here.

pub mod constants;
pub mod hearing;
pub mod message;
pub mod modulation;
pub mod radio_calc;
pub mod server_settings;
pub mod short_guid;
pub mod version;
pub mod voice_packet;

pub use hearing::{can_hear_transmission, candidate_score, HearResult, RxRadio};
pub use message::{Ambient, LatLng, MessageType, NetworkMessage, Radio, RadioInfo, SrClient, Transponder};
pub use modulation::Modulation;
pub use server_settings::{ServerSettingsKey, SyncedServerSettings};
pub use short_guid::ShortGuid;
pub use version::{ProtocolVersion, MIN_PROTOCOL_VERSION};
pub use voice_packet::{is_ping, FrequencySegment, VoicePacket, VoicePacketError};
