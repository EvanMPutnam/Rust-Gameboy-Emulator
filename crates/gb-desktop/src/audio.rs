use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SampleRate, Stream, StreamConfig};

use crate::error::DesktopError;

const CORE_SAMPLE_RATE_HZ: u32 = 48_000;
const RESAMPLER_CHUNK_SAMPLES: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioStats {
    pub queued_samples: usize,
    pub dropped_samples: u64,
    pub underflow_events: u64,
    pub enqueued_samples: u64,
    pub dequeued_samples: u64,
    pub callback_requested_samples: u64,
    pub callback_calls: u64,
}

#[derive(Debug)]
struct AudioQueueState {
    queue: VecDeque<i16>,
    max_samples: usize,
    dropped_samples: u64,
    underflow_events: u64,
    enqueued_samples: u64,
    dequeued_samples: u64,
    callback_requested_samples: u64,
    callback_calls: u64,
}

#[derive(Debug, Clone)]
pub struct AudioQueue {
    inner: Arc<Mutex<AudioQueueState>>,
}

impl AudioQueue {
    #[must_use]
    pub fn new(max_samples: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(AudioQueueState {
                queue: VecDeque::new(),
                max_samples,
                dropped_samples: 0,
                underflow_events: 0,
                enqueued_samples: 0,
                dequeued_samples: 0,
                callback_requested_samples: 0,
                callback_calls: 0,
            })),
        }
    }

    pub fn enqueue(&self, samples: &[i16]) {
        let Ok(mut state) = self.inner.lock() else {
            return;
        };

        state.enqueued_samples = state
            .enqueued_samples
            .wrapping_add(usize_to_u64(samples.len()));

        if state.max_samples == 0 {
            state.dropped_samples = state
                .dropped_samples
                .wrapping_add(usize_to_u64(samples.len()));
            return;
        }

        for &sample in samples {
            if state.queue.len() >= state.max_samples {
                let _ = state.queue.pop_front();
                state.dropped_samples = state.dropped_samples.wrapping_add(1);
            }
            state.queue.push_back(sample);
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn drain(&self, max_samples: usize) -> Vec<i16> {
        let Ok(mut state) = self.inner.lock() else {
            return Vec::new();
        };

        if state.queue.is_empty() {
            state.underflow_events = state.underflow_events.wrapping_add(1);
            return Vec::new();
        }

        let count = max_samples.min(state.queue.len());
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            if let Some(sample) = state.queue.pop_front() {
                out.push(sample);
            }
        }
        state.dequeued_samples = state.dequeued_samples.wrapping_add(usize_to_u64(out.len()));
        out
    }

    pub fn dequeue_into(&self, output: &mut [i16]) {
        if output.is_empty() {
            return;
        }

        let Ok(mut state) = self.inner.lock() else {
            output.fill(0);
            return;
        };

        let available = state.queue.len().min(output.len());
        for slot in output.iter_mut().take(available) {
            if let Some(sample) = state.queue.pop_front() {
                *slot = sample;
            }
        }
        for slot in output.iter_mut().skip(available) {
            *slot = 0;
        }

        state.dequeued_samples = state.dequeued_samples.wrapping_add(usize_to_u64(available));

        if available < output.len() {
            state.underflow_events = state.underflow_events.wrapping_add(1);
        }
    }

    pub fn record_callback_request(&self, requested_samples: usize) {
        let Ok(mut state) = self.inner.lock() else {
            return;
        };

        state.callback_calls = state.callback_calls.wrapping_add(1);
        state.callback_requested_samples = state
            .callback_requested_samples
            .wrapping_add(usize_to_u64(requested_samples));
    }

    #[must_use]
    pub fn stats(&self) -> AudioStats {
        let Ok(state) = self.inner.lock() else {
            return AudioStats {
                queued_samples: 0,
                dropped_samples: 0,
                underflow_events: 0,
                enqueued_samples: 0,
                dequeued_samples: 0,
                callback_requested_samples: 0,
                callback_calls: 0,
            };
        };

        AudioStats {
            queued_samples: state.queue.len(),
            dropped_samples: state.dropped_samples,
            underflow_events: state.underflow_events,
            enqueued_samples: state.enqueued_samples,
            dequeued_samples: state.dequeued_samples,
            callback_requested_samples: state.callback_requested_samples,
            callback_calls: state.callback_calls,
        }
    }
}

pub struct AudioBackend {
    _stream: Stream,
}

impl AudioBackend {
    pub fn start(queue: &AudioQueue, muted: bool) -> Result<Option<Self>, DesktopError> {
        if muted {
            return Ok(None);
        }

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(DesktopError::AudioNoDevice)?;
        let default_config = device
            .default_output_config()
            .map_err(DesktopError::AudioDefaultConfig)?;
        let channels = usize::from(default_config.channels()).max(1);
        let default_stream_config: StreamConfig = default_config.config();

        let stream = if default_stream_config.sample_rate.0 == CORE_SAMPLE_RATE_HZ {
            build_stream(
                &device,
                default_config.sample_format(),
                &default_stream_config,
                channels,
                queue.clone(),
            )?
        } else {
            let mut preferred_stream_config = default_stream_config.clone();
            preferred_stream_config.sample_rate = SampleRate(CORE_SAMPLE_RATE_HZ);

            match build_stream(
                &device,
                default_config.sample_format(),
                &preferred_stream_config,
                channels,
                queue.clone(),
            ) {
                Ok(stream) => stream,
                Err(DesktopError::AudioStreamBuild(
                    cpal::BuildStreamError::StreamConfigNotSupported,
                )) => build_stream(
                    &device,
                    default_config.sample_format(),
                    &default_stream_config,
                    channels,
                    queue.clone(),
                )?,
                Err(err) => return Err(err),
            }
        };

        stream.play().map_err(DesktopError::AudioStreamPlay)?;
        Ok(Some(Self { _stream: stream }))
    }
}

fn build_stream(
    device: &Device,
    sample_format: SampleFormat,
    stream_config: &StreamConfig,
    channels: usize,
    queue: AudioQueue,
) -> Result<Stream, DesktopError> {
    match sample_format {
        SampleFormat::I16 => build_i16_stream(device, stream_config, channels, queue),
        SampleFormat::U16 => build_u16_stream(device, stream_config, channels, queue),
        SampleFormat::F32 => build_f32_stream(device, stream_config, channels, queue),
        other => Err(DesktopError::AudioUnsupportedSampleFormat(format!(
            "{other:?}"
        ))),
    }
}

fn build_i16_stream(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    queue: AudioQueue,
) -> Result<Stream, DesktopError> {
    let mut resampler = StereoResampler::new(CORE_SAMPLE_RATE_HZ, config.sample_rate.0);
    device
        .build_output_stream(
            config,
            move |data: &mut [i16], _| write_i16_data(data, channels, &queue, &mut resampler),
            |error| eprintln!("audio stream error: {error}"),
            None,
        )
        .map_err(DesktopError::AudioStreamBuild)
}

fn build_u16_stream(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    queue: AudioQueue,
) -> Result<Stream, DesktopError> {
    let mut resampler = StereoResampler::new(CORE_SAMPLE_RATE_HZ, config.sample_rate.0);
    device
        .build_output_stream(
            config,
            move |data: &mut [u16], _| write_u16_data(data, channels, &queue, &mut resampler),
            |error| eprintln!("audio stream error: {error}"),
            None,
        )
        .map_err(DesktopError::AudioStreamBuild)
}

fn build_f32_stream(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    queue: AudioQueue,
) -> Result<Stream, DesktopError> {
    let mut resampler = StereoResampler::new(CORE_SAMPLE_RATE_HZ, config.sample_rate.0);
    device
        .build_output_stream(
            config,
            move |data: &mut [f32], _| write_f32_data(data, channels, &queue, &mut resampler),
            |error| eprintln!("audio stream error: {error}"),
            None,
        )
        .map_err(DesktopError::AudioStreamBuild)
}

fn write_i16_data(
    data: &mut [i16],
    channels: usize,
    queue: &AudioQueue,
    resampler: &mut StereoResampler,
) {
    if channels == 0 {
        data.fill(0);
        return;
    }

    queue.record_callback_request(data.len());

    let frame_count = data.len() / channels;
    for frame_index in 0..frame_count {
        let (left, right) = resampler.next_frame(queue);
        for channel in 0..channels {
            data[(frame_index * channels) + channel] = map_stereo_to_channel(left, right, channel);
        }
    }

    for sample in data.iter_mut().skip(frame_count.saturating_mul(channels)) {
        *sample = 0;
    }
}

fn write_u16_data(
    data: &mut [u16],
    channels: usize,
    queue: &AudioQueue,
    resampler: &mut StereoResampler,
) {
    if channels == 0 {
        data.fill(0);
        return;
    }

    queue.record_callback_request(data.len());

    let frame_count = data.len() / channels;
    for frame_index in 0..frame_count {
        let (left, right) = resampler.next_frame(queue);
        for channel in 0..channels {
            let signed = map_stereo_to_channel(left, right, channel);
            let shifted = i32::from(signed) + 32_768;
            data[(frame_index * channels) + channel] = u16::try_from(shifted).unwrap_or_default();
        }
    }

    for sample in data.iter_mut().skip(frame_count.saturating_mul(channels)) {
        *sample = 0;
    }
}

fn write_f32_data(
    data: &mut [f32],
    channels: usize,
    queue: &AudioQueue,
    resampler: &mut StereoResampler,
) {
    if channels == 0 {
        data.fill(0.0);
        return;
    }

    queue.record_callback_request(data.len());

    let frame_count = data.len() / channels;
    for frame_index in 0..frame_count {
        let (left, right) = resampler.next_frame(queue);
        for channel in 0..channels {
            let signed = map_stereo_to_channel(left, right, channel);
            let normalized = f32::from(signed) / f32::from(i16::MAX);
            data[(frame_index * channels) + channel] = normalized.clamp(-1.0, 1.0);
        }
    }

    for sample in data.iter_mut().skip(frame_count.saturating_mul(channels)) {
        *sample = 0.0;
    }
}

#[derive(Debug)]
struct StereoResampler {
    source_rate_hz: u32,
    output_rate_hz: u32,
    phase_accum: u64,
    current_left: i16,
    current_right: i16,
    primed: bool,
    cache: VecDeque<i16>,
}

impl StereoResampler {
    fn new(source_rate_hz: u32, output_rate_hz: u32) -> Self {
        Self {
            source_rate_hz,
            output_rate_hz: output_rate_hz.max(1),
            phase_accum: 0,
            current_left: 0,
            current_right: 0,
            primed: false,
            cache: VecDeque::new(),
        }
    }

    fn next_frame(&mut self, queue: &AudioQueue) -> (i16, i16) {
        if !self.primed {
            let (left, right) = self.dequeue_stereo_frame(queue);
            self.current_left = left;
            self.current_right = right;
            self.primed = true;
        }

        let out = (self.current_left, self.current_right);
        self.phase_accum = self
            .phase_accum
            .wrapping_add(u64::from(self.source_rate_hz));

        while self.phase_accum >= u64::from(self.output_rate_hz) {
            self.phase_accum -= u64::from(self.output_rate_hz);
            let (left, right) = self.dequeue_stereo_frame(queue);
            self.current_left = left;
            self.current_right = right;
        }

        out
    }

    fn dequeue_stereo_frame(&mut self, queue: &AudioQueue) -> (i16, i16) {
        if self.cache.len() < 2 {
            let mut pulled = vec![0i16; RESAMPLER_CHUNK_SAMPLES.max(2)];
            queue.dequeue_into(&mut pulled);
            self.cache.extend(pulled);
        }

        let left = self.cache.pop_front().unwrap_or_default();
        let right = self.cache.pop_front().unwrap_or_default();
        (left, right)
    }
}

#[must_use]
fn map_stereo_to_channel(left: i16, right: i16, channel: usize) -> i16 {
    if channel == 0 {
        return left;
    }
    if channel == 1 {
        return right;
    }
    if (channel & 1) == 0 { left } else { right }
}

#[must_use]
fn usize_to_u64(value: usize) -> u64 {
    match u64::try_from(value) {
        Ok(as_u64) => as_u64,
        Err(_) => u64::MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AudioQueue, StereoResampler, map_stereo_to_channel, write_i16_data, write_u16_data,
    };

    #[test]
    fn overflow_drops_oldest_samples_and_underflow_is_counted() {
        let queue = AudioQueue::new(4);
        queue.enqueue(&[1, 2, 3, 4, 5, 6]);

        assert_eq!(queue.stats().queued_samples, 4);
        assert_eq!(queue.stats().dropped_samples, 2);

        let drained = queue.drain(8);
        assert_eq!(drained, vec![3, 4, 5, 6]);

        let empty = queue.drain(1);
        assert!(empty.is_empty());
        assert_eq!(queue.stats().underflow_events, 1);
    }

    #[test]
    fn callback_write_pads_with_silence_on_underflow() {
        let queue = AudioQueue::new(8);
        queue.enqueue(&[10, -10, 20, -20]);

        let mut out = [0i16; 6];
        let mut resampler = StereoResampler::new(48_000, 48_000);
        write_i16_data(&mut out, 2, &queue, &mut resampler);
        assert_eq!(out, [10, -10, 20, -20, 0, 0]);
        assert_eq!(queue.stats().underflow_events, 1);
        assert_eq!(queue.stats().callback_requested_samples, 6);
    }

    #[test]
    fn stereo_mapping_handles_multi_channel_devices() {
        assert_eq!(map_stereo_to_channel(7, -7, 0), 7);
        assert_eq!(map_stereo_to_channel(7, -7, 1), -7);
        assert_eq!(map_stereo_to_channel(7, -7, 2), 7);
        assert_eq!(map_stereo_to_channel(7, -7, 3), -7);
    }

    #[test]
    fn u16_mapping_uses_shifted_signed_range() {
        let queue = AudioQueue::new(8);
        queue.enqueue(&[-32_768, 32_767, 0, 0]);

        let mut out = [0u16; 4];
        let mut resampler = StereoResampler::new(48_000, 48_000);
        write_u16_data(&mut out, 2, &queue, &mut resampler);

        assert_eq!(out[0], 0);
        assert_eq!(out[1], u16::MAX);
    }

    #[test]
    fn resampler_downsamples_when_output_rate_is_lower() {
        let queue = AudioQueue::new(64);
        queue.enqueue(&[100, 100, 200, 200, 300, 300, 400, 400]);

        let mut resampler = StereoResampler::new(48_000, 24_000);
        let first = resampler.next_frame(&queue);
        let second = resampler.next_frame(&queue);

        assert_eq!(first, (100, 100));
        assert_eq!(second, (300, 300));
    }
}
