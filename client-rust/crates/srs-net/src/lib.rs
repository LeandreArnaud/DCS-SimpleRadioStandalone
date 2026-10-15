//! Client networking for SRS, on tokio.
//!
//! - [`tcp`]: the control session (`TCPClientHandler.cs`): SYNC handshake,
//!   client list, server settings, EAM, idle timeout, version checks.
//! - [`shared`]: state shared with the rest of the client (connected
//!   clients, synced server settings).
//! - [`udp`]: voice transport with ping / ready / 42 s watchdog.

pub mod shared;
pub mod tcp;
pub mod udp;

pub use shared::SharedState;
pub use tcp::{NetCommand, NetEvent, SessionConfig, TcpHandle, TcpSession};
pub use udp::{UdpConfig, UdpHandle, UdpVoice, VoipEvent};
