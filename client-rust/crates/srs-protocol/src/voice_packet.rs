//! UDP voice packet, byte-exact port of `Common/Models/UDPVoicePacket.cs`.
//!
//! Layout (all integers little-endian):
//!
//! | bytes  | field                                   |
//! |--------|-----------------------------------------|
//! | 2      | u16 total packet length                 |
//! | 2      | u16 audio length (`A`)                  |
//! | 2      | u16 frequency segment length (`N * 10`) |
//! | A      | Opus frame                              |
//! | N * 10 | f64 frequency Hz, u8 modulation, u8 key |
//! | 4      | u32 unit id                             |
//! | 8      | u64 packet number                       |
//! | 1      | u8 retransmission (hop) count           |
//! | 22     | original client GUID (ASCII)            |
//! | 22     | client GUID of the current sender       |
//!
//! The C# decoder reads the two GUIDs and the hop count **from the end of the
//! buffer**, and the rest from the front. This port does the same, so a packet
//! with trailing garbage decodes the way the C# client would see it.

use crate::short_guid::{ShortGuid, SHORT_GUID_LEN};

pub const PACKET_HEADER_LEN: usize = 6;
pub const FREQUENCY_SEGMENT_LEN: usize = 10;
/// unit id + packet number + hop count + two GUIDs.
pub const FIXED_SEGMENT_LEN: usize = 4 + 8 + 1 + SHORT_GUID_LEN + SHORT_GUID_LEN;
/// Shortest datagram `UDPClientAudioProcessor` accepts as voice: header +
/// fixed part + one frequency segment (the audio may be empty).
pub const MIN_VOICE_PACKET_LEN: usize = PACKET_HEADER_LEN + FIXED_SEGMENT_LEN + FREQUENCY_SEGMENT_LEN;

/// Default modulation byte written when the sender did not provide one.
/// The C# code uses `4` (HAVEQUICK), not AM; kept for parity.
pub const DEFAULT_MODULATION_BYTE: u8 = 4;

/// A datagram of exactly 22 bytes is a keep-alive ping (the sender's GUID).
pub fn is_ping(datagram: &[u8]) -> bool {
    datagram.len() == SHORT_GUID_LEN
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrequencySegment {
    pub freq_hz: f64,
    /// Raw modulation byte, see [`crate::Modulation::from_u8`].
    pub modulation: u8,
    /// Encryption key, `0` = clear.
    pub encryption: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoicePacket {
    pub audio: Vec<u8>,
    pub frequencies: Vec<FrequencySegment>,
    pub unit_id: u32,
    /// Starts at 1 and increments per packet sent by a client.
    pub packet_number: u64,
    pub retransmission_count: u8,
    /// GUID of the client that originally transmitted (kept across relays).
    pub original_client_guid: ShortGuid,
    /// GUID of the client that sent this datagram.
    pub client_guid: ShortGuid,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum VoicePacketError {
    #[error("datagram too short ({0} bytes)")]
    TooShort(usize),
    #[error("declared lengths exceed the datagram ({0} bytes declared, {1} available)")]
    LengthMismatch(usize, usize),
    #[error("audio ({0}) or frequency ({1}) length exceeds u16")]
    Overflow(usize, usize),
}

impl VoicePacket {
    /// Total encoded length for this packet.
    pub fn encoded_len(&self) -> usize {
        PACKET_HEADER_LEN
            + self.audio.len()
            + self.frequencies.len() * FREQUENCY_SEGMENT_LEN
            + FIXED_SEGMENT_LEN
    }

    pub fn encode(&self) -> Result<Vec<u8>, VoicePacketError> {
        let mut out = Vec::with_capacity(self.encoded_len());
        self.encode_into(&mut out)?;
        Ok(out)
    }

    /// Append the encoded packet to `out` (cleared first).
    pub fn encode_into(&self, out: &mut Vec<u8>) -> Result<(), VoicePacketError> {
        let freq_len = self.frequencies.len() * FREQUENCY_SEGMENT_LEN;
        let total = self.encoded_len();
        if self.audio.len() > u16::MAX as usize || freq_len > u16::MAX as usize || total > u16::MAX as usize {
            return Err(VoicePacketError::Overflow(self.audio.len(), freq_len));
        }
        out.clear();
        out.reserve(total);
        out.extend_from_slice(&(total as u16).to_le_bytes());
        out.extend_from_slice(&(self.audio.len() as u16).to_le_bytes());
        out.extend_from_slice(&(freq_len as u16).to_le_bytes());
        out.extend_from_slice(&self.audio);
        for f in &self.frequencies {
            out.extend_from_slice(&f.freq_hz.to_le_bytes());
            out.push(f.modulation);
            out.push(f.encryption);
        }
        out.extend_from_slice(&self.unit_id.to_le_bytes());
        out.extend_from_slice(&self.packet_number.to_le_bytes());
        out.push(self.retransmission_count);
        out.extend_from_slice(self.original_client_guid.as_bytes());
        out.extend_from_slice(self.client_guid.as_bytes());
        debug_assert_eq!(out.len(), total);
        Ok(())
    }

    /// Decode a datagram. Mirrors `DecodeVoicePacket(bytes, decode: true)`.
    pub fn decode(buf: &[u8]) -> Result<Self, VoicePacketError> {
        let n = buf.len();
        if n < PACKET_HEADER_LEN + FIXED_SEGMENT_LEN {
            return Err(VoicePacketError::TooShort(n));
        }
        // Tail: read from the end like the C# code.
        let client_guid = guid_at(buf, n - SHORT_GUID_LEN);
        let original_client_guid = guid_at(buf, n - 2 * SHORT_GUID_LEN);
        let retransmission_count = buf[n - 2 * SHORT_GUID_LEN - 1];

        // Head.
        let audio_len = u16::from_le_bytes([buf[2], buf[3]]) as usize;
        let freq_len = u16::from_le_bytes([buf[4], buf[5]]) as usize;
        let freq_count = freq_len / FREQUENCY_SEGMENT_LEN;
        let fixed_offset = PACKET_HEADER_LEN + audio_len + freq_len;
        if fixed_offset + 4 + 8 > n {
            return Err(VoicePacketError::LengthMismatch(fixed_offset + 12, n));
        }
        let audio = buf[PACKET_HEADER_LEN..PACKET_HEADER_LEN + audio_len].to_vec();
        let mut frequencies = Vec::with_capacity(freq_count);
        let mut off = PACKET_HEADER_LEN + audio_len;
        for _ in 0..freq_count {
            let mut fb = [0u8; 8];
            fb.copy_from_slice(&buf[off..off + 8]);
            frequencies.push(FrequencySegment {
                freq_hz: f64::from_le_bytes(fb),
                modulation: buf[off + 8],
                encryption: buf[off + 9],
            });
            off += FREQUENCY_SEGMENT_LEN;
        }
        let unit_id = u32::from_le_bytes(buf[fixed_offset..fixed_offset + 4].try_into().unwrap());
        let packet_number = u64::from_le_bytes(buf[fixed_offset + 4..fixed_offset + 12].try_into().unwrap());
        Ok(Self {
            audio,
            frequencies,
            unit_id,
            packet_number,
            retransmission_count,
            original_client_guid,
            client_guid,
        })
    }

    /// Declared total length from the header, if the buffer is long enough.
    pub fn declared_len(buf: &[u8]) -> Option<usize> {
        (buf.len() >= 2).then(|| u16::from_le_bytes([buf[0], buf[1]]) as usize)
    }
}

fn guid_at(buf: &[u8], at: usize) -> ShortGuid {
    let mut g = [0u8; SHORT_GUID_LEN];
    g.copy_from_slice(&buf[at..at + SHORT_GUID_LEN]);
    ShortGuid::from_raw(g)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_len_matches_csharp() {
        assert_eq!(FIXED_SEGMENT_LEN, 57);
        assert_eq!(MIN_VOICE_PACKET_LEN, 73);
    }

    #[test]
    fn rejects_short_buffers() {
        assert_eq!(
            VoicePacket::decode(&[0u8; 10]),
            Err(VoicePacketError::TooShort(10))
        );
        // Header says 1000 bytes of audio but only 63 bytes are present.
        let mut b = vec![0u8; 63];
        b[2] = 0xE8;
        b[3] = 0x03;
        assert!(matches!(
            VoicePacket::decode(&b),
            Err(VoicePacketError::LengthMismatch(..))
        ));
    }
}
