//! Mirror of `Common/Models/Player/Modulation.cs`. Serialized as an integer.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Modulation {
    Am = 0,
    Fm = 1,
    Intercom = 2,
    #[default]
    Disabled = 3,
    HaveQuick = 4,
    Satcom = 5,
    Mids = 6,
    Sincgars = 7,
}

impl Modulation {
    pub const ALL: [Modulation; 8] = [
        Modulation::Am,
        Modulation::Fm,
        Modulation::Intercom,
        Modulation::Disabled,
        Modulation::HaveQuick,
        Modulation::Satcom,
        Modulation::Mids,
        Modulation::Sincgars,
    ];

    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Exact conversion; `None` for values outside 0..=7.
    pub const fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0 => Modulation::Am,
            1 => Modulation::Fm,
            2 => Modulation::Intercom,
            3 => Modulation::Disabled,
            4 => Modulation::HaveQuick,
            5 => Modulation::Satcom,
            6 => Modulation::Mids,
            7 => Modulation::Sincgars,
            _ => return None,
        })
    }

    /// Lenient conversion used for JSON: System.Text.Json accepts any integer
    /// for a C# enum, so an unknown value must not abort message decoding.
    /// Unknown values map to `Disabled`, which can never be heard or sent.
    pub fn from_i64_lossy(v: i64) -> Self {
        u8::try_from(v)
            .ok()
            .and_then(Self::from_u8)
            .unwrap_or(Modulation::Disabled)
    }

    /// Display name as used in the C# UI / presets.
    pub const fn name(self) -> &'static str {
        match self {
            Modulation::Am => "AM",
            Modulation::Fm => "FM",
            Modulation::Intercom => "INTERCOM",
            Modulation::Disabled => "DISABLED",
            Modulation::HaveQuick => "HAVEQUICK",
            Modulation::Satcom => "SATCOM",
            Modulation::Mids => "MIDS",
            Modulation::Sincgars => "SINCGARS",
        }
    }
}

impl Serialize for Modulation {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(self.as_u8())
    }
}

impl<'de> Deserialize<'de> for Modulation {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // Accept floats too: Lua-produced JSON sometimes writes `1.0`.
        let v = f64::deserialize(d)?;
        Ok(Modulation::from_i64_lossy(v as i64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_roundtrip_as_int() {
        assert_eq!(serde_json::to_string(&Modulation::HaveQuick).unwrap(), "4");
        assert_eq!(
            serde_json::from_str::<Modulation>("7").unwrap(),
            Modulation::Sincgars
        );
        assert_eq!(serde_json::from_str::<Modulation>("1.0").unwrap(), Modulation::Fm);
        assert_eq!(
            serde_json::from_str::<Modulation>("42").unwrap(),
            Modulation::Disabled
        );
    }
}
