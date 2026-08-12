use crate::error::AppError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const TARGET_SAMPLE_RATE: u32 = 16_000;
/// Peak amplitude (against i16 full scale) below which a capture is dead air
/// rather than quiet speech. Deliberately far under speech level: a quiet but
/// real recording must still reach Groq instead of being rejected locally.
/// 64 / 32768 is roughly -54 dBFS, i.e. below a typical mic noise floor.
const SILENCE_PEAK_THRESHOLD: f64 = 64.0;
const MIN_SAMPLES: usize = TARGET_SAMPLE_RATE as usize / 2;

/// Held only to keep the capture alive; dropping it stops the stream.
pub struct SendStream(#[allow(dead_code)] cpal::Stream);

unsafe impl Send for SendStream {}
unsafe impl Sync for SendStream {}

pub struct ActiveRecording {
    stream: SendStream,
    samples: Arc<Mutex<Vec<i16>>>,
    /// Loudest sample seen since the last level poll, so the UI can show a live
    /// meter without copying the whole buffer. Reset by `input_level`.
    recent_peak: Arc<AtomicI32>,
    sample_rate: u32,
    device: String,
    started_at: Instant,
}

#[derive(Default)]
pub struct RecorderState {
    active: Mutex<Option<ActiveRecording>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MicrophoneDevice {
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecordingSummary {
    pub duration_ms: u128,
    pub sample_rate: u32,
    pub samples: usize,
    pub rms: f64,
    pub peak: f64,
    pub device: String,
}

/// Live capture telemetry for the settings-panel meter. `peak` is normalised
/// 0.0–1.0 so the UI never has to know about i16 full scale.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct InputLevel {
    pub recording: bool,
    pub peak: f64,
    pub device: String,
    pub samples: usize,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone)]
pub struct CapturedAudio {
    pub wav_bytes: Vec<u8>,
    pub summary: RecordingSummary,
}

fn device_name(device: &cpal::Device) -> String {
    device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "Unknown microphone".to_string())
}

pub fn list_input_devices() -> Result<Vec<MicrophoneDevice>, AppError> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .map(|device| device_name(&device));

    let devices = host
        .input_devices()
        .map_err(|e| AppError::Audio(format!("Cannot enumerate microphones: {e}")))?;

    let mut result = Vec::new();
    for device in devices {
        let name = device_name(&device);
        let is_default = default_name.as_ref().is_some_and(|default| default == &name);
        result.push(MicrophoneDevice { name, is_default });
    }
    Ok(result)
}

/// Honour the microphone chosen in settings, but never fail because of it: a
/// renamed or unplugged device falls back to the system default instead of
/// blocking dictation entirely.
fn resolve_input_device(
    host: &cpal::Host,
    preferred: Option<&str>,
) -> Result<cpal::Device, AppError> {
    if let Some(name) = preferred.map(str::trim).filter(|name| !name.is_empty()) {
        if let Ok(devices) = host.input_devices() {
            for device in devices {
                if device_name(&device) == name {
                    return Ok(device);
                }
            }
        }
        eprintln!(
            "OrcaVoice: microphone \"{name}\" is unavailable; falling back to the system default."
        );
    }

    host.default_input_device().ok_or_else(|| {
        AppError::Audio(
            "No microphone found. Plug one in, or enable it in Windows sound settings.".to_string(),
        )
    })
}

pub fn start_recording(state: &RecorderState, preferred: Option<&str>) -> Result<(), AppError> {
    let mut guard = state
        .active
        .lock()
        .map_err(|_| AppError::Audio("Recorder lock is poisoned.".to_string()))?;
    if guard.is_some() {
        // Idempotent: already recording, nothing to do.
        return Ok(());
    }

    let host = cpal::default_host();
    let device = resolve_input_device(&host, preferred)?;
    let device_label = device_name(&device);
    let supported_config = device.default_input_config().map_err(|e| {
        AppError::Audio(format!(
            "Cannot read the configuration of \"{device_label}\": {e}"
        ))
    })?;
    let sample_rate = supported_config.sample_rate();
    let channels = supported_config.channels();
    let stream_config = supported_config.config();
    let stream_config_i8 = stream_config;
    let stream_config_i16 = stream_config;
    let stream_config_i32 = stream_config;
    let stream_config_f32 = stream_config;
    let samples = Arc::new(Mutex::new(Vec::<i16>::new()));
    let recent_peak = Arc::new(AtomicI32::new(0));
    let err_fn = |err: cpal::Error| eprintln!("OrcaVoice microphone stream error: {err}");

    let stream = match supported_config.sample_format() {
        cpal::SampleFormat::I8 => {
            let cb_samples = samples.clone();
            let cb_peak = recent_peak.clone();
            device.build_input_stream::<i8, _, _>(
                stream_config_i8,
                move |data: &[i8], _| {
                    push_samples(
                        data.iter().map(|v| (*v as i16) << 8),
                        channels,
                        &cb_samples,
                        &cb_peak,
                    );
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let cb_samples = samples.clone();
            let cb_peak = recent_peak.clone();
            device.build_input_stream::<i16, _, _>(
                stream_config_i16,
                move |data: &[i16], _| {
                    push_samples(data.iter().copied(), channels, &cb_samples, &cb_peak);
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I32 => {
            let cb_samples = samples.clone();
            let cb_peak = recent_peak.clone();
            device.build_input_stream::<i32, _, _>(
                stream_config_i32,
                move |data: &[i32], _| {
                    push_samples(
                        data.iter().map(|v| (*v >> 16) as i16),
                        channels,
                        &cb_samples,
                        &cb_peak,
                    );
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::F32 => {
            let cb_samples = samples.clone();
            let cb_peak = recent_peak.clone();
            device.build_input_stream::<f32, _, _>(
                stream_config_f32,
                move |data: &[f32], _| {
                    push_samples(
                        data.iter().map(|v| float_to_i16(*v)),
                        channels,
                        &cb_samples,
                        &cb_peak,
                    );
                },
                err_fn,
                None,
            )
        }
        other => {
            return Err(AppError::Audio(format!(
                "Unsupported microphone sample format: {other:?}"
            )))
        }
    }
    .map_err(|e| {
        AppError::Audio(format!(
            "Cannot open \"{device_label}\": {e}. Another app may own the microphone."
        ))
    })?;

    stream.play().map_err(|e| {
        AppError::Audio(format!("Cannot start capture on \"{device_label}\": {e}"))
    })?;

    *guard = Some(ActiveRecording {
        stream: SendStream(stream),
        samples,
        recent_peak,
        sample_rate,
        device: device_label,
        started_at: Instant::now(),
    });
    Ok(())
}

pub fn stop_recording(state: &RecorderState) -> Result<CapturedAudio, AppError> {
    let active = state
        .active
        .lock()
        .map_err(|_| AppError::Audio("Recorder lock is poisoned.".to_string()))?
        .take()
        .ok_or_else(|| AppError::Audio("Recording is not active.".to_string()))?;
    drop(active.stream);

    let duration_ms = active.started_at.elapsed().as_millis();
    let samples = active
        .samples
        .lock()
        .map_err(|_| AppError::Audio("Audio sample lock is poisoned.".to_string()))?
        .clone();

    let normalized = downsample_to_16k(&samples, active.sample_rate);
    if normalized.len() < MIN_SAMPLES {
        return Err(AppError::Audio(
            "Recording is too short. Speak for at least half a second.".to_string(),
        ));
    }

    let rms = calculate_rms(&normalized);
    let peak = calculate_peak(&normalized);
    if peak < SILENCE_PEAK_THRESHOLD {
        return Err(AppError::Audio(silence_message(&active.device, peak, rms)));
    }

    let wav_bytes = encode_wav(&normalized, TARGET_SAMPLE_RATE)?;
    Ok(CapturedAudio {
        wav_bytes,
        summary: RecordingSummary {
            duration_ms,
            sample_rate: TARGET_SAMPLE_RATE,
            samples: normalized.len(),
            rms,
            peak: normalize_amplitude(peak),
            device: active.device,
        },
    })
}

/// The old message ("check microphone permissions and input level") named no
/// device and no number, so a blocked mic, a muted mic and a wrong default
/// device all looked identical. Report what was measured and where.
fn silence_message(device: &str, peak: f64, rms: f64) -> String {
    if peak <= 1.0 {
        format!(
            "No audio at all from \"{device}\" — every sample was zero. \
             Windows hands blocked apps pure silence: turn on Settings › Privacy & security › \
             Microphone › \"Let desktop apps access your microphone\", or pick another \
             microphone below."
        )
    } else {
        format!(
            "\"{device}\" is effectively silent (peak {:.1}%, level {rms:.0}). \
             It is likely muted or set to the wrong input — raise its level in Windows sound \
             settings, or pick another microphone below.",
            normalize_amplitude(peak) * 100.0
        )
    }
}

pub fn is_recording(state: &RecorderState) -> bool {
    state
        .active
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false)
}

/// Peak since the previous call, so the settings panel can prove whether audio
/// is arriving *before* the user records something that turns out to be silent.
pub fn input_level(state: &RecorderState) -> InputLevel {
    let Ok(guard) = state.active.lock() else {
        return InputLevel::default();
    };
    let Some(active) = guard.as_ref() else {
        return InputLevel::default();
    };
    let peak = active.recent_peak.swap(0, Ordering::Relaxed) as f64;
    let samples = active.samples.lock().map(|s| s.len()).unwrap_or(0);
    InputLevel {
        recording: true,
        peak: normalize_amplitude(peak),
        device: active.device.clone(),
        samples,
        elapsed_ms: active.started_at.elapsed().as_millis(),
    }
}

pub fn cancel_recording(state: &RecorderState) {
    if let Ok(mut guard) = state.active.lock() {
        *guard = None;
    }
}

fn push_samples<I>(data: I, channels: u16, samples: &Arc<Mutex<Vec<i16>>>, peak: &AtomicI32)
where
    I: Iterator<Item = i16>,
{
    let mut guard = match samples.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    let start = guard.len();
    if channels <= 1 {
        guard.extend(data);
    } else {
        let channel_count = channels as usize;
        let mut frame = Vec::with_capacity(channel_count);
        for sample in data {
            frame.push(sample);
            if frame.len() == channel_count {
                let sum: i32 = frame.iter().map(|v| *v as i32).sum();
                guard.push((sum / channel_count as i32) as i16);
                frame.clear();
            }
        }
    }

    let chunk_peak = guard[start..]
        .iter()
        .map(|sample| (*sample as i32).abs())
        .max()
        .unwrap_or(0);
    peak.fetch_max(chunk_peak, Ordering::Relaxed);
}

fn float_to_i16(value: f32) -> i16 {
    let clamped = value.clamp(-1.0, 1.0);
    (clamped * i16::MAX as f32) as i16
}

fn downsample_to_16k(samples: &[i16], input_rate: u32) -> Vec<i16> {
    if input_rate == TARGET_SAMPLE_RATE {
        return samples.to_vec();
    }
    let ratio = input_rate as f64 / TARGET_SAMPLE_RATE as f64;
    let output_len = (samples.len() as f64 / ratio).floor() as usize;
    let mut output = Vec::with_capacity(output_len);
    for index in 0..output_len {
        let source_index = (index as f64 * ratio).floor() as usize;
        if let Some(sample) = samples.get(source_index) {
            output.push(*sample);
        }
    }
    output
}

fn calculate_rms(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let square_sum: f64 = samples
        .iter()
        .map(|sample| {
            let value = *sample as f64;
            value * value
        })
        .sum();
    (square_sum / samples.len() as f64).sqrt()
}

fn calculate_peak(samples: &[i16]) -> f64 {
    samples
        .iter()
        .map(|sample| (*sample as i32).abs())
        .max()
        .unwrap_or(0) as f64
}

fn normalize_amplitude(amplitude: f64) -> f64 {
    (amplitude / i16::MAX as f64).clamp(0.0, 1.0)
}

fn encode_wav(samples: &[i16], sample_rate: u32) -> Result<Vec<u8>, AppError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)
            .map_err(|e| AppError::Audio(format!("Cannot create WAV writer: {e}")))?;
        for sample in samples {
            writer
                .write_sample(*sample)
                .map_err(|e| AppError::Audio(format!("Cannot write WAV sample: {e}")))?;
        }
        writer
            .finalize()
            .map_err(|e| AppError::Audio(format!("Cannot finalize WAV data: {e}")))?;
    }
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_encoding_has_riff_header() {
        let samples = vec![0_i16; TARGET_SAMPLE_RATE as usize];
        let wav = encode_wav(&samples, TARGET_SAMPLE_RATE).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
    }

    #[test]
    fn rms_detects_non_silence() {
        let samples = vec![1000_i16; 1000];
        assert!(calculate_rms(&samples) > 0.0);
    }

    /// Regression: speech with normal pauses has a low *average* level, so the
    /// old mean-RMS gate rejected perfectly good dictation as "silent".
    #[test]
    fn quiet_speech_with_pauses_is_not_silent() {
        // One short quiet word inside a second of silence: clearly audible, but
        // its mean level sits under the old RMS gate of 80.
        let mut samples = vec![0_i16; 16_000];
        for (index, sample) in samples.iter_mut().enumerate().take(200) {
            *sample = if index % 2 == 0 { 600 } else { -600 };
        }
        assert!(calculate_rms(&samples) < 80.0, "old gate would have rejected this");
        assert!(calculate_peak(&samples) >= SILENCE_PEAK_THRESHOLD);
    }

    #[test]
    fn dead_air_is_still_rejected() {
        let samples = vec![0_i16; 16_000];
        assert!(calculate_peak(&samples) < SILENCE_PEAK_THRESHOLD);
    }

    #[test]
    fn all_zero_capture_names_the_windows_privacy_switch() {
        let message = silence_message("Yeti Nano", 0.0, 0.0);
        assert!(message.contains("Yeti Nano"));
        assert!(message.contains("Privacy"));
    }

    #[test]
    fn quiet_capture_reports_the_measured_level() {
        let message = silence_message("Yeti Nano", 30.0, 12.0);
        assert!(message.contains("peak 0.1%"), "unexpected message: {message}");
    }

    #[test]
    fn peak_tracking_survives_multichannel_downmix() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let peak = AtomicI32::new(0);
        push_samples([1200_i16, 1200, -800, -800].into_iter(), 2, &samples, &peak);
        assert_eq!(samples.lock().unwrap().len(), 2);
        assert_eq!(peak.load(Ordering::Relaxed), 1200);
    }
}
