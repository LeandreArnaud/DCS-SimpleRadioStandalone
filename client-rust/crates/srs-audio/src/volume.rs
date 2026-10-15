//! Port of `Common/Audio/Utility/VolumeConversionHelper.cs`.

const LOG_2_DB: f64 = 8.685_889_638_065_037; // 20 / ln(10)
const DB_2_LOG: f64 = 0.115_129_254_649_702_28; // ln(10) / 20

pub fn linear_to_decibels(lin: f64) -> f64 {
    lin.ln() * LOG_2_DB
}

pub fn decibels_to_linear(db: f64) -> f64 {
    (db * DB_2_LOG).exp()
}

/// `ConvertFloatToDB`: `0` is replaced by a tiny value to avoid `-inf`.
pub fn float_to_db(linear: f32) -> f64 {
    let linear = if linear == 0.0 { 0.000_015_8_f32 } else { linear };
    linear_to_decibels(f64::from(linear))
}

/// `ConvertVolumeSliderToScale`: slider 0..1 → -30 dB .. +28 dB → linear.
pub fn slider_to_scale(volume: f32) -> f32 {
    let db = -30.0 + (28.0 - -30.0) * f64::from(volume);
    (db / 20.0 * std::f64::consts::LN_10).exp() as f32
}

/// `CalculateRMS(short[])` in dBFS; `-96.6` for silence.
pub fn rms_db_i16(pcm: &[i16]) -> f64 {
    let mut sum = 0.0f64;
    for &s in pcm {
        if s != 0 {
            let v = f64::from(s) / 32768.0;
            sum += v * v;
        }
    }
    if sum == 0.0 {
        return -96.6;
    }
    let rms = (sum / pcm.len() as f64).sqrt();
    if rms == 0.0 {
        0.0
    } else {
        20.0 * rms.log10()
    }
}

/// `CalculateRMS(float[])` in dBFS.
pub fn rms_db_f32(pcm: &[f32]) -> f64 {
    let sum: f64 = pcm.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    if sum == 0.0 {
        return -96.6;
    }
    let rms = (sum / pcm.len() as f64).sqrt();
    if rms == 0.0 {
        0.0
    } else {
        20.0 * rms.log10()
    }
}

/// `ConvertRadioVolumeSlider`: radio volume slider (0..1, or above with
/// `allow_boost`) → linear gain through dB.
pub fn radio_volume_slider(slider: f32, allow_boost: bool) -> f32 {
    let mut v = f64::from(slider);
    if v <= 0.0 {
        v = 0.000_001;
    } else if v > 1.0 && !allow_boost {
        v = 1.0;
    }
    let db = 20.0 * v.log10();
    10f64.powf(db / 20.0) as f32
}

/// `GetTargetdB`: reduce `current_db` by a factor expressed as a ratio.
pub fn target_db(current_db: f64, reduction_factor: f64) -> f64 {
    if reduction_factor <= 0.0 {
        return -144.0;
    }
    if reduction_factor >= 1.0 {
        return current_db;
    }
    current_db + 10.0 * reduction_factor.log2()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions() {
        assert!(
            (slider_to_scale(0.514) - 0.978).abs() < 0.005,
            "{}",
            slider_to_scale(0.514)
        );
        assert!((radio_volume_slider(1.0, false) - 1.0).abs() < 1e-6);
        assert!((radio_volume_slider(0.5, false) - 0.5).abs() < 1e-6);
        assert!((radio_volume_slider(2.0, false) - 1.0).abs() < 1e-6);
        assert!((radio_volume_slider(2.0, true) - 2.0).abs() < 1e-5);
        assert_eq!(rms_db_i16(&[0; 10]), -96.6);
        assert!((rms_db_f32(&[1.0; 10]) - 0.0).abs() < 1e-9);
        assert!((float_to_db(1.0)).abs() < 1e-9);
        assert_eq!(target_db(-20.0, 1.0), -20.0);
        assert!((target_db(-20.0, 0.5) - -30.0).abs() < 1e-9);
    }
}
