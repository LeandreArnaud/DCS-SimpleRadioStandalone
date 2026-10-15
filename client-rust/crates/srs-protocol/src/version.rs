//! Four-part version strings as produced by .NET `System.Version`.
//!
//! The server refuses any client whose `Version` parses below
//! [`MIN_PROTOCOL_VERSION`] (see `Common/Helpers/UpdaterChecker.cs`). The
//! client applies the same rule to the server's SYNC reply.

use std::fmt;
use std::str::FromStr;

/// A `major.minor[.build[.revision]]` version. Missing trailing parts compare
/// as `0` (the .NET type uses `-1`, which only matters for `1.9` vs `1.9.0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProtocolVersion(pub [u32; 4]);

/// `UpdaterChecker.MINIMUM_PROTOCOL_VERSION`.
pub const MIN_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion([1, 9, 0, 0]);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum VersionParseError {
    #[error("version must have 2 to 4 numeric parts, got {0:?}")]
    PartCount(usize),
    #[error("invalid numeric part {0:?}")]
    Part(String),
}

impl ProtocolVersion {
    pub const fn new(major: u32, minor: u32, build: u32, revision: u32) -> Self {
        Self([major, minor, build, revision])
    }

    /// True when this version satisfies the minimum protocol version.
    pub fn is_compatible(&self) -> bool {
        *self >= MIN_PROTOCOL_VERSION
    }
}

impl FromStr for ProtocolVersion {
    type Err = VersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.trim().split('.').collect();
        if parts.len() < 2 || parts.len() > 4 {
            return Err(VersionParseError::PartCount(parts.len()));
        }
        let mut out = [0u32; 4];
        for (i, p) in parts.iter().enumerate() {
            out[i] = p
                .trim()
                .parse()
                .map_err(|_| VersionParseError::Part(p.to_string()))?;
        }
        Ok(Self(out))
    }
}

impl fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c, d] = self.0;
        write!(f, "{a}.{b}.{c}.{d}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_compare() {
        let v: ProtocolVersion = "2.4.1.0".parse().unwrap();
        assert_eq!(v, ProtocolVersion::new(2, 4, 1, 0));
        assert!(v.is_compatible());
        assert!(!"1.0.0.0".parse::<ProtocolVersion>().unwrap().is_compatible());
        assert!("1.9.0.0".parse::<ProtocolVersion>().unwrap().is_compatible());
        assert!("1.9.0.1".parse::<ProtocolVersion>().unwrap().is_compatible());
        assert!(!"1.8.99.99".parse::<ProtocolVersion>().unwrap().is_compatible());
        assert_eq!(
            "2.4".parse::<ProtocolVersion>().unwrap(),
            ProtocolVersion::new(2, 4, 0, 0)
        );
        assert!("2".parse::<ProtocolVersion>().is_err());
        assert!("a.b".parse::<ProtocolVersion>().is_err());
        assert_eq!(v.to_string(), "2.4.1.0");
    }
}
