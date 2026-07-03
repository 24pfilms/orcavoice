use crate::error::AppError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const TARGET_SAMPLE_RATE: u32 = 16_000;
const SILENCE_RMS_THRESHOLD: f64 = 80.0;
const MIN_SAMPLES: usize = TARGET_SAMPLE_RATE as usize / 2;

pub struct SendStream(cpal::Stream);

unsafe impl Send for SendStream {}
unsafe impl Sync for SendStream {}

pub struct ActiveRecording {
    stream: SendStream,
    samples: Arc<Mutex<Vec<i16>>>,
    sample_rate: u32,
    channels: u16,
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

pub fn start_recording(state: &RecorderState) -> Result<(), AppError> {
    let mut guard = state
        .active
        .lock()
        .map_err(|_| AppError::Audio("Recorder lock is poisoned.".to_string()))?;
    if guard.is_some() {
        // Idempotent: already recording, nothing to do.
        return Ok(());
    }

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| AppError::Audio("No default microphone input device found.".to_string()))?;
    let supported_config = device
        .default_input_config()
        .map_err(|e| AppError::Audio(format!("Cannot read default microphone config: {e}")))?;
    let sample_rate = supported_config.sample_rate();
    let channels = supported_config.channels();
    let stream_config = supported_config.config();
    let stream_config_i8 = stream_config.clone();
    let stream_config_i16 = stream_config.clone();
    let stream_config_i32 = stream_config.clone();
    let stream_config_f32 = stream_config.clone();
    let samples = Arc::new(Mutex::new(Vec::<i16>::new()));
    let err_fn = |err: cpal::Error| eprintln!("OrcaVoice microphone stream error: {err}");

    let stream = match supported_config.sample_format() {
        cpal::SampleFormat::I8 => {
            let cb_samples = samples.clone();
            device.build_input_stream::<i8, _, _>(
                stream_config_i8,
                move |data: &[i8], _| {
                    push_samples(
                        data.iter().map(|v| (*v as i16) << 8),
                        channels,
                        &cb_samples,
                    );
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let cb_samples = samples.clone();
            device.build_input_stream::<i16, _, _>(
                stream_config_i16,
                move |data: &[i16], _| {
                    push_samples(data.iter().copied(), channels, &cb_samples);
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I32 => {
            let cb_samples = samples.clone();
            device.build_input_stream::<i32, _, _>(
                stream_config_i32,
                move |data: &[i32], _| {
                    push_samples(data.iter().map(|v| (*v >> 16) as i16), channels, &cb_samples);
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::F32 => {
            let cb_samples = samples.clone();
            device.build_input_stream::<f32, _, _>(
                stream_config_f32,
                move |data: &[f32], _| {
                    push_samples(
                        data.iter().map(|v| float_to_i16(*v)),
                        channels,
                        &cb_samples,
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
    .map_err(|e| AppError::Audio(format!("Cannot build microphone stream: {e}")))?;

    stream
        .play()
        .map_err(|e| AppError::Audio(format!("Cannot start microphone stream: {e}")))?;

    *guard = Some(ActiveRecording {
        stream: SendStream(stream),
        samples,
        sample_rate,
        channels,
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

    if samples.len() < MIN_SAMPLES {
        return Err(AppError::Audio(
            "Recording is too short. Speak for at least half a second.".to_string(),
        ));
    }

    let normalized = downsample_to_16k(&samples, active.sample_rate);
    let rms = calculate_rms(&normalized);
    if rms < SILENCE_RMS_THRESHOLD {
        return Err(AppError::Audio(
            "Recording looks silent. Check microphone permissions and input level.".to_string(),
        ));
    }

    let wav_bytes = encode_wav(&normalized, TARGET_SAMPLE_RATE)?;
    Ok(CapturedAudio {
        wav_bytes,
        summary: RecordingSummary {
            duration_ms,
            sample_rate: TARGET_SAMPLE_RATE,
            samples: normalized.len(),
            rms,
        },
    })
}

pub fn is_recording(state: &RecorderState) -> bool {
    state
        .active
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false)
}

pub fn cancel_recording(state: &RecorderState) {
    if let Ok(mut guard) = state.active.lock() {
        *guard = None;
    }
}

fn push_samples<I>(data: I, channels: u16, samples: &Arc<Mutex<Vec<i16>>>)
where
    I: Iterator<Item = i16>,
{
    let mut guard = match samples.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    if channels <= 1 {
        guard.extend(data);
        return;
    }

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
        assert!(calculate_rms(&samples) > SILENCE_RMS_THRESHOLD);
    }
}
