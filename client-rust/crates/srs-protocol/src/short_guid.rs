//! 22-character client identifier, identical to `Common/Models/ShortGuid.cs`.
//!
//! The C# code does `Convert.ToBase64String(guid.ToByteArray())`, replaces
//! `/` with `_` and `+` with `-`, and keeps the first 22 characters (the two
//! `=` padding characters are dropped). A new one is generated at every client
//! start; the value is never persisted.
//!
//! On the wire it travels as the JSON `ClientGuid` string and as the 22 ASCII
//! bytes at the end of every UDP voice packet. A datagram of exactly 22 bytes
//! is the UDP "ping".

use std::fmt;
use std::str::FromStr;

/// Length in bytes/characters of a short GUID.
pub const SHORT_GUID_LEN: usize = 22;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Opaque 22-byte identifier. Compared and hashed byte-wise.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShortGuid([u8; SHORT_GUID_LEN]);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ShortGuidError {
    #[error("short guid must be {SHORT_GUID_LEN} characters, got {0}")]
    Length(usize),
    #[error("short guid contains a character outside the base64url alphabet")]
    Alphabet,
}

impl ShortGuid {
    /// Fresh random identifier (what the C# client does at startup).
    pub fn new_random() -> Self {
        let bytes: [u8; 16] = rand::random();
        Self::from_guid_bytes(&bytes)
    }

    /// Encode 16 GUID bytes exactly like `Convert.ToBase64String` + the
    /// `/`→`_`, `+`→`-` replacement. The byte order is whatever the caller
    /// provides (for .NET parity that is `Guid.ToByteArray()`, i.e. the first
    /// three fields little-endian).
    pub fn from_guid_bytes(guid: &[u8; 16]) -> Self {
        let mut out = [0u8; SHORT_GUID_LEN];
        let mut o = 0;
        // 15 bytes = 5 full groups of 3 -> 20 chars; last byte -> 2 chars.
        for chunk in guid[..15].chunks(3) {
            let n = (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2]);
            out[o] = ALPHABET[(n >> 18) as usize & 63];
            out[o + 1] = ALPHABET[(n >> 12) as usize & 63];
            out[o + 2] = ALPHABET[(n >> 6) as usize & 63];
            out[o + 3] = ALPHABET[n as usize & 63];
            o += 4;
        }
        let last = u32::from(guid[15]) << 16;
        out[20] = ALPHABET[(last >> 18) as usize & 63];
        out[21] = ALPHABET[(last >> 12) as usize & 63];
        Self(out)
    }

    /// Decode back to the 16 GUID bytes. Trailing bits of the last character
    /// are ignored, as `Convert.FromBase64String` does.
    pub fn to_guid_bytes(&self) -> Result<[u8; 16], ShortGuidError> {
        fn val(c: u8) -> Result<u32, ShortGuidError> {
            ALPHABET
                .iter()
                .position(|&a| a == c)
                .map(|p| p as u32)
                .ok_or(ShortGuidError::Alphabet)
        }
        let s = &self.0;
        let mut out = [0u8; 16];
        let mut o = 0;
        for chunk in s[..20].chunks(4) {
            let n = (val(chunk[0])? << 18) | (val(chunk[1])? << 12) | (val(chunk[2])? << 6) | val(chunk[3])?;
            out[o] = (n >> 16) as u8;
            out[o + 1] = (n >> 8) as u8;
            out[o + 2] = n as u8;
            o += 3;
        }
        let n = (val(s[20])? << 18) | (val(s[21])? << 12);
        out[15] = (n >> 16) as u8;
        Ok(out)
    }

    /// Wrap raw bytes received from the network without validation. The C#
    /// side does `Encoding.ASCII.GetString` and never validates either.
    pub const fn from_raw(bytes: [u8; SHORT_GUID_LEN]) -> Self {
        Self(bytes)
    }

    /// Parse the JSON/string form. Validates length and alphabet.
    pub fn parse(s: &str) -> Result<Self, ShortGuidError> {
        let b = s.as_bytes();
        if b.len() != SHORT_GUID_LEN {
            return Err(ShortGuidError::Length(b.len()));
        }
        if !b.iter().all(|c| ALPHABET.contains(c)) {
            return Err(ShortGuidError::Alphabet);
        }
        let mut out = [0u8; SHORT_GUID_LEN];
        out.copy_from_slice(b);
        Ok(Self(out))
    }

    pub const fn as_bytes(&self) -> &[u8; SHORT_GUID_LEN] {
        &self.0
    }

    /// String form. Falls back to `?` for non-ASCII bytes (only possible for
    /// values built with [`ShortGuid::from_raw`]).
    pub fn as_str(&self) -> std::borrow::Cow<'_, str> {
        match std::str::from_utf8(&self.0) {
            Ok(s) => std::borrow::Cow::Borrowed(s),
            Err(_) => std::borrow::Cow::Owned(
                self.0
                    .iter()
                    .map(|&b| if b.is_ascii() { b as char } else { '?' })
                    .collect(),
            ),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.0.iter().all(|c| ALPHABET.contains(c))
    }
}

impl FromStr for ShortGuid {
    type Err = ShortGuidError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for ShortGuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_str())
    }
}

impl fmt::Debug for ShortGuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ShortGuid({})", self.as_str())
    }
}

impl serde::Serialize for ShortGuid {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for ShortGuid {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <String as serde::Deserialize>::deserialize(d)?;
        ShortGuid::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_guid_is_all_a() {
        let g = ShortGuid::from_guid_bytes(&[0u8; 16]);
        assert_eq!(g.as_str(), "AAAAAAAAAAAAAAAAAAAAAA");
        assert_eq!(g.to_guid_bytes().unwrap(), [0u8; 16]);
    }

    #[test]
    fn csharp_test_vector_roundtrips() {
        // Value used by DCS-SR-CommonTests/Network/UDPVoicePacketTests.cs
        let g = ShortGuid::parse("ufYS_WlLVkmFPjqCgxz6GA").unwrap();
        let bytes = g.to_guid_bytes().unwrap();
        assert_eq!(ShortGuid::from_guid_bytes(&bytes), g);
        assert_eq!(g.to_string(), "ufYS_WlLVkmFPjqCgxz6GA");
    }

    #[test]
    fn known_vector_with_url_safe_chars() {
        // Guid bytes chosen so that the base64 output contains both '+' and '/':
        // 0xFB 0xFF 0xBF ... -> "+/+/" in standard base64 -> "-_-_" here.
        let bytes = [0xFB, 0xFF, 0xBF, 0xFB, 0xFF, 0xBF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let g = ShortGuid::from_guid_bytes(&bytes);
        assert_eq!(g.as_str(), "-_-_-_-_AAAAAAAAAAAAAA");
        assert_eq!(g.to_guid_bytes().unwrap(), bytes);
    }

    #[test]
    fn random_is_valid_and_unique() {
        let a = ShortGuid::new_random();
        let b = ShortGuid::new_random();
        assert!(a.is_valid());
        assert_ne!(a, b);
        assert_eq!(ShortGuid::parse(&a.to_string()).unwrap(), a);
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(ShortGuid::parse("short"), Err(ShortGuidError::Length(5)));
        assert_eq!(
            ShortGuid::parse("ufYS_WlLVkmFPjqCgxz6G="),
            Err(ShortGuidError::Alphabet)
        );
    }

    #[test]
    fn serde_json_string() {
        let g = ShortGuid::parse("ufYS_WlLVkmFPjqCgxz6GA").unwrap();
        let js = serde_json::to_string(&g).unwrap();
        assert_eq!(js, "\"ufYS_WlLVkmFPjqCgxz6GA\"");
        let back: ShortGuid = serde_json::from_str(&js).unwrap();
        assert_eq!(back, g);
    }
}
