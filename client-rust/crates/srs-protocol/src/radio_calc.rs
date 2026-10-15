//! Port of `Common/Helpers/RadioCalculator.cs`: distance and range maths used
//! by the client for `DISTANCE_ENABLED` / `LOS_ENABLED`.

use crate::message::LatLng;

/// AM transmissions at or below this frequency ignore line of sight (HF
/// skywave). 30 MHz.
pub const HF_FREQUENCY_LOS_IGNORED_HZ: f64 = 3e7;
pub const TRANSMISSION_POWER_DBM: f64 = 40.0;
pub const RX_ANTENNA_GAIN: f64 = 1.0;
pub const TX_ANTENNA_GAIN: f64 = 1.0;
/// Friis equation rearranged for a 40 dBm transmitter and -90 dBm receiver.
#[allow(clippy::inconsistent_digit_grouping)]
pub const MAGIC_PARTIALLY_SOLVED_FRIIS: f64 = 995_267.9264;
pub const RX_SENSITIVITY_DBM: f64 = -90.0;
pub const SPEED_OF_LIGHT: f64 = 299_792_458.0;
pub const MHZ: f64 = 1e6;

pub fn frequency_to_wavelength(frequency_hz: f64) -> f64 {
    SPEED_OF_LIGHT / frequency_hz
}

/// Received power in dBm at `distance_m` (unused by the client, kept for parity).
pub fn friis_received_power(distance_m: f64, frequency_hz: f64) -> f64 {
    TRANSMISSION_POWER_DBM + RX_ANTENNA_GAIN + TX_ANTENNA_GAIN
        - 20.0 * (4.0 * std::f64::consts::PI * distance_m / frequency_to_wavelength(frequency_hz)).log10()
}

/// Maximum range in metres for the fixed 40 dBm transmitter.
pub fn friis_maximum_transmission_range(frequency_hz: f64) -> f64 {
    MAGIC_PARTIALLY_SOLVED_FRIIS * frequency_to_wavelength(frequency_hz) / std::f64::consts::PI
}

/// True when `distance_m` is within the Friis maximum range.
pub fn can_hear_at_distance(distance_m: f64, frequency_hz: f64) -> bool {
    friis_maximum_transmission_range(frequency_hz) > distance_m
}

pub fn degree_to_radian(angle: f64) -> f64 {
    std::f64::consts::PI * angle / 180.0
}

/// Haversine great-circle distance combined with the altitude difference.
/// Reproduces the C# guard: identical latitude **or** longitude returns the
/// altitude difference only.
pub fn distance_haversine(me: &LatLng, other: &LatLng) -> f64 {
    if me.lat == other.lat || me.lng == other.lng {
        return (me.alt - other.alt).abs();
    }
    const R: f64 = 6_371e3;
    let phi1 = degree_to_radian(me.lat);
    let phi2 = degree_to_radian(other.lat);
    let d_phi = degree_to_radian(other.lat - me.lat);
    let d_lambda = degree_to_radian(other.lng - me.lng);
    let a = (d_phi / 2.0).sin().powi(2) + phi1.cos() * phi2.cos() * (d_lambda / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    pythag_distance(R * c, me.alt - other.alt).abs()
}

fn pythag_distance(distance: f64, height: f64) -> f64 {
    let height = height.abs();
    let distance = distance.abs();
    if height == 0.0 {
        return distance;
    }
    (distance * distance + height * height).sqrt()
}

/// Link16 / MIDS channel mapping (`RadioCalculator.Link16`).
pub mod link16 {
    use super::MHZ;

    pub const BASE_FREQ: f64 = 1030.0 * MHZ;
    pub const CHANNEL_SEPARATION: f64 = 0.1 * MHZ;
    pub const MIN_FREQ: f64 = BASE_FREQ;
    pub const MAX_FREQ: f64 = BASE_FREQ + 127.0 * CHANNEL_SEPARATION;

    pub fn validate_channel(channel: i32) -> i32 {
        channel.clamp(0, 127)
    }

    pub fn validate_frequency(frequency: f64) -> f64 {
        frequency.clamp(MIN_FREQ, MAX_FREQ)
    }

    pub fn channel_to_frequency(channel: i32) -> f64 {
        BASE_FREQ + CHANNEL_SEPARATION * f64::from(validate_channel(channel))
    }

    pub fn frequency_to_channel(frequency: f64) -> i32 {
        ((validate_frequency(frequency) - BASE_FREQ) / CHANNEL_SEPARATION) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_scales_with_wavelength() {
        // UHF 251 MHz: about 378 km; VHF 30 MHz: about 3165 km.
        let uhf = friis_maximum_transmission_range(251e6);
        let vhf = friis_maximum_transmission_range(30e6);
        assert!((uhf - 378_400.0).abs() < 1_000.0, "{uhf}");
        assert!((vhf - 3_165_900.0).abs() < 5_000.0, "{vhf}");
        assert!(can_hear_at_distance(300_000.0, 251e6));
        assert!(!can_hear_at_distance(400_000.0, 251e6));
    }

    #[test]
    fn haversine_and_guards() {
        let a = LatLng::new(43.0, 41.0, 1000.0);
        let b = LatLng::new(43.0, 42.0, 0.0);
        // Same latitude: C# shortcut returns the altitude difference.
        assert_eq!(distance_haversine(&a, &b), 1000.0);
        let c = LatLng::new(44.0, 42.0, 0.0);
        let d = distance_haversine(&a, &c);
        assert!((d - 137_000.0).abs() < 2_000.0, "{d}");
    }

    #[test]
    fn link16() {
        assert_eq!(link16::channel_to_frequency(0), 1030e6);
        assert_eq!(link16::frequency_to_channel(1042.7e6), 127);
        assert_eq!(link16::channel_to_frequency(200), link16::MAX_FREQ);
    }
}
