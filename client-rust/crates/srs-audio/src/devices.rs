//! Audio device I/O through cpal (WASAPI on Windows, CoreAudio on macOS,
//! ALSA on Linux): output (rendering the engine) and capture (feeding the
//! encoder thread through a channel).
//!
//! The engine renders 48 kHz stereo. The stream is opened at 48 kHz when the
//! device accepts it; otherwise a linear resampler bridges to the device
//! rate (good enough for radio voice; rubato can replace it later).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use srs_protocol::constants::OUTPUT_SAMPLE_RATE;

use crate::engine::SharedEngine;

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("no output device available")]
    NoOutputDevice,
    #[error("no supported output configuration")]
    NoConfig,
    #[error("cpal: {0}")]
    Cpal(#[from] cpal::Error),
}

/// Description of a device for settings / UI. On Windows `id` is the
/// WASAPI endpoint id (`{0.0.0.00000000}.{guid}`), the string the C# client
/// stores in `AudioInputDeviceId` / `AudioOutputDeviceId`.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

fn describe(d: &cpal::Device, default_name: Option<&str>) -> Option<DeviceInfo> {
    let name = d.description().ok()?.name().to_string();
    let id = d.id().map(|i| i.id().to_string()).unwrap_or_default();
    Some(DeviceInfo {
        id,
        is_default: Some(name.as_str()) == default_name,
        name,
    })
}

pub fn input_devices() -> Vec<DeviceInfo> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.description().ok().map(|d| d.name().to_string()));
    host.input_devices()
        .map(|it| it.filter_map(|d| describe(&d, default_name.as_deref())).collect())
        .unwrap_or_default()
}

/// Stored id → device: `""` / `"default"` / unknown id → the default device.
/// A plain device name is accepted too (tests, non-Windows hosts).
fn find_device<I: Iterator<Item = cpal::Device>>(devices: I, selector: Option<&str>) -> Option<cpal::Device> {
    let sel = selector
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("default"))?;
    devices.into_iter().find(|d| {
        d.id().map(|i| i.id() == sel).unwrap_or(false)
            || d.description().map(|desc| desc.name() == sel).unwrap_or(false)
    })
}

/// A running capture stream. Chunks of interleaved f32 samples are sent to
/// `sink` from the audio thread (`try_send`, dropped when the consumer lags).
pub struct InputStream {
    _stream: cpal::Stream,
    pub sample_rate: u32,
    pub channels: u16,
}

impl InputStream {
    pub fn start(
        device_name: Option<&str>,
        sink: std::sync::mpsc::SyncSender<Vec<f32>>,
    ) -> Result<Self, DeviceError> {
        let host = cpal::default_host();
        let device = find_device(host.input_devices()?, device_name)
            .or_else(|| host.default_input_device())
            .ok_or(DeviceError::NoOutputDevice)?;
        log::info!(
            "Input device: {}",
            device
                .description()
                .map(|d| d.name().to_string())
                .unwrap_or_default()
        );
        let supported = device.default_input_config()?;
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let sample_rate = config.sample_rate;
        let channels = config.channels;
        log::info!("Input stream: {sample_rate} Hz, {channels} ch, {sample_format:?}");
        let err_fn = |e| log::error!("Input stream error: {e}");
        let stream = match sample_format {
            SampleFormat::F32 => device.build_input_stream(
                config,
                move |data: &[f32], _| {
                    let _ = sink.try_send(data.to_vec());
                },
                err_fn,
                None,
            )?,
            SampleFormat::I16 => device.build_input_stream(
                config,
                move |data: &[i16], _| {
                    let v: Vec<f32> = data.iter().map(|&s| f32::from(s) / 32768.0).collect();
                    let _ = sink.try_send(v);
                },
                err_fn,
                None,
            )?,
            other => {
                log::error!("Unsupported input sample format {other:?}");
                return Err(DeviceError::NoConfig);
            }
        };
        stream.play()?;
        Ok(Self {
            _stream: stream,
            sample_rate,
            channels,
        })
    }
}

pub fn output_devices() -> Vec<DeviceInfo> {
    let host = cpal::default_host();
    let default_name = host
        .default_output_device()
        .and_then(|d| d.description().ok().map(|d| d.name().to_string()));
    host.output_devices()
        .map(|it| it.filter_map(|d| describe(&d, default_name.as_deref())).collect())
        .unwrap_or_default()
}

/// A running output stream. Drop to stop.
pub struct OutputStream {
    _stream: cpal::Stream,
    pub sample_rate: u32,
    pub channels: u16,
}

impl OutputStream {
    /// Open the default (or named) output device and start rendering the engine.
    pub fn start(engine: SharedEngine, device_name: Option<&str>) -> Result<Self, DeviceError> {
        let host = cpal::default_host();
        let device = find_device(host.output_devices()?, device_name)
            .or_else(|| host.default_output_device())
            .ok_or(DeviceError::NoOutputDevice)?;
        log::info!(
            "Output device: {}",
            device
                .description()
                .map(|d| d.name().to_string())
                .unwrap_or_default()
        );

        // Prefer 48 kHz, f32 or i16, 1 or 2 channels.
        let default = device.default_output_config()?;
        let mut chosen: Option<cpal::SupportedStreamConfig> = None;
        for cfg in device.supported_output_configs()? {
            if cfg.channels() > 2 || cfg.channels() == 0 {
                continue;
            }
            let min = cfg.min_sample_rate();
            let max = cfg.max_sample_rate();
            if min <= OUTPUT_SAMPLE_RATE
                && OUTPUT_SAMPLE_RATE <= max
                && matches!(cfg.sample_format(), SampleFormat::F32 | SampleFormat::I16)
            {
                let candidate = cfg.with_sample_rate(OUTPUT_SAMPLE_RATE);
                let better = match &chosen {
                    None => true,
                    Some(c) => candidate.channels() == 2 && c.channels() != 2,
                };
                if better {
                    chosen = Some(candidate);
                }
            }
        }
        let supported = chosen.unwrap_or(default);
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let sample_rate = config.sample_rate;
        let channels = config.channels;
        log::info!("Output stream: {sample_rate} Hz, {channels} ch, {sample_format:?}");

        let mut renderer = Renderer::new(engine, sample_rate, channels);
        let err_fn = |e| log::error!("Output stream error: {e}");
        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                config,
                move |data: &mut [f32], _| renderer.fill_f32(data),
                err_fn,
                None,
            )?,
            SampleFormat::I16 => device.build_output_stream(
                config,
                move |data: &mut [i16], _| renderer.fill_i16(data),
                err_fn,
                None,
            )?,
            other => {
                log::error!("Unsupported output sample format {other:?}");
                return Err(DeviceError::NoConfig);
            }
        };
        stream.play()?;
        Ok(Self {
            _stream: stream,
            sample_rate,
            channels,
        })
    }
}

/// Pulls 48 kHz stereo from the engine and adapts channels / rate.
struct Renderer {
    engine: SharedEngine,
    rate: u32,
    channels: usize,
    stereo48: Vec<f32>,
    // Linear resampler state (only when rate != 48000).
    pending: Vec<f32>, // interleaved stereo at 48 kHz not yet consumed
    pos: f64,
    tmp: Vec<f32>,
}

impl Renderer {
    fn new(engine: SharedEngine, rate: u32, channels: u16) -> Self {
        Self {
            engine,
            rate,
            channels: channels as usize,
            stereo48: Vec::new(),
            pending: Vec::new(),
            pos: 0.0,
            tmp: Vec::new(),
        }
    }

    /// Produce `frames` stereo frames at the device rate into `self.tmp`.
    #[allow(clippy::needless_range_loop)]
    fn render_frames(&mut self, frames: usize) {
        if self.rate == OUTPUT_SAMPLE_RATE {
            self.tmp.resize(frames * 2, 0.0);
            let mut tmp = std::mem::take(&mut self.tmp);
            self.engine.lock().render(&mut tmp);
            self.tmp = tmp;
            return;
        }
        // Linear interpolation from 48 kHz.
        let step = OUTPUT_SAMPLE_RATE as f64 / self.rate as f64;
        let needed_src = (self.pos + step * frames as f64).ceil() as usize + 2;
        let have = self.pending.len() / 2;
        if have < needed_src {
            let add = needed_src - have;
            self.stereo48.resize(add * 2, 0.0);
            let mut s48 = std::mem::take(&mut self.stereo48);
            self.engine.lock().render(&mut s48);
            self.pending.extend_from_slice(&s48);
            self.stereo48 = s48;
        }
        self.tmp.resize(frames * 2, 0.0);
        for i in 0..frames {
            let p = self.pos + step * i as f64;
            let idx = p as usize;
            let frac = (p - idx as f64) as f32;
            for ch in 0..2 {
                let a = self.pending[idx * 2 + ch];
                let b = self.pending.get((idx + 1) * 2 + ch).copied().unwrap_or(a);
                self.tmp[i * 2 + ch] = a + (b - a) * frac;
            }
        }
        let consumed_f = self.pos + step * frames as f64;
        let consumed = consumed_f as usize;
        self.pos = consumed_f - consumed as f64;
        self.pending.drain(..consumed * 2);
    }

    #[allow(clippy::needless_range_loop)]
    fn fill_f32(&mut self, data: &mut [f32]) {
        let frames = data.len() / self.channels;
        self.render_frames(frames);
        match self.channels {
            2 => data.copy_from_slice(&self.tmp[..frames * 2]),
            1 => {
                for i in 0..frames {
                    data[i] = 0.5 * (self.tmp[2 * i] + self.tmp[2 * i + 1]);
                }
            }
            _ => data.iter_mut().for_each(|s| *s = 0.0),
        }
    }

    #[allow(clippy::needless_range_loop)]
    fn fill_i16(&mut self, data: &mut [i16]) {
        let frames = data.len() / self.channels;
        self.render_frames(frames);
        for i in 0..frames {
            for ch in 0..self.channels {
                let v = match self.channels {
                    2 => self.tmp[2 * i + ch],
                    _ => 0.5 * (self.tmp[2 * i] + self.tmp[2 * i + 1]),
                };
                data[i * self.channels + ch] = (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            }
        }
    }
}
