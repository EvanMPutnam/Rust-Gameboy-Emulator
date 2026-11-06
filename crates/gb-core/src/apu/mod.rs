#![allow(clippy::struct_excessive_bools)] // Channel state mirrors hardware latch bits for clarity.

use crate::Cycles;
use crate::error::ApuError;
use serde::{Deserialize, Serialize};

const CPU_HZ: u64 = 4_194_304;
const FRAME_SEQUENCER_PERIOD: Cycles = 8_192;
const SAMPLE_RATE: u64 = 48_000;
const MIX_OUTPUT_GAIN: i32 = 48;
const HPF_SHIFT: u32 = 11;
const FNV64_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV64_PRIME: u64 = 0x0000_0001_0000_01B3;
const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
struct FrameSequencer {
    cycles: Cycles,
    step: u8,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
struct FrameEvents {
    clock_length: bool,
    clock_sweep: bool,
    clock_envelope: bool,
    sequencer_steps: u8,
}

impl FrameSequencer {
    fn tick(&mut self, cycles: Cycles) -> FrameEvents {
        let mut events = FrameEvents::default();
        self.cycles = self.cycles.wrapping_add(cycles);

        while self.cycles >= FRAME_SEQUENCER_PERIOD {
            self.cycles -= FRAME_SEQUENCER_PERIOD;
            events.sequencer_steps = events.sequencer_steps.wrapping_add(1);

            // Run the current step, then advance: step 0 is the first one after power-on.
            events.clock_length = events.clock_length || matches!(self.step, 0 | 2 | 4 | 6);
            events.clock_sweep = events.clock_sweep || self.step == 2 || self.step == 6;
            events.clock_envelope = events.clock_envelope || self.step == 7;
            self.step = (self.step + 1) & 0x07;
        }

        events
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
struct SweepUnit {
    period: u8,
    counter: u8,
    shift: u8,
    negate: bool,
    enabled: bool,
    shadow_freq: u16,
    #[serde(skip)]
    negate_used: bool,
}

impl SweepUnit {
    fn write_nr10(&mut self, value: u8, ch1: &mut SquareChannel) {
        self.period = (value >> 4) & 0x07;
        let negate = (value & 0x08) != 0;
        // Leaving negate mode after a negated calculation disables the channel.
        if self.negate && !negate && self.negate_used {
            ch1.enabled = false;
        }
        self.negate = negate;
        self.shift = value & 0x07;
    }

    fn trigger(&mut self, ch1: &mut SquareChannel) {
        self.negate_used = false;
        self.shadow_freq = ch1.freq;
        self.counter = if self.period == 0 { 8 } else { self.period };
        self.enabled = self.period != 0 || self.shift != 0;

        if self.shift != 0 {
            let next = self.calculate_next();
            if next > 2_047 {
                self.enabled = false;
                ch1.enabled = false;
            }
        }
    }

    fn clock(&mut self, ch1: &mut SquareChannel) {
        if !self.enabled || !ch1.enabled {
            return;
        }

        if self.counter > 0 {
            self.counter -= 1;
        }
        if self.counter != 0 {
            return;
        }

        self.counter = if self.period == 0 { 8 } else { self.period };
        if self.period == 0 {
            return;
        }

        let next = self.calculate_next();
        if next > 2_047 {
            self.enabled = false;
            ch1.enabled = false;
            return;
        }

        if self.shift == 0 {
            return;
        }

        self.shadow_freq = next;
        ch1.freq = next;
        ch1.timer = square_timer_period(ch1.freq);

        if self.calculate_next() > 2_047 {
            self.enabled = false;
            ch1.enabled = false;
        }
    }

    fn calculate_next(&mut self) -> u16 {
        let delta = self.shadow_freq >> self.shift;
        if self.negate {
            self.negate_used = true;
            self.shadow_freq.checked_sub(delta).unwrap_or(2_048)
        } else {
            self.shadow_freq.wrapping_add(delta)
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
struct SquareChannel {
    enabled: bool,
    length_counter: u16,
    length_enabled: bool,
    duty: u8,
    duty_pos: u8,
    timer: i32,
    freq: u16,
    initial_volume: u8,
    current_volume: u8,
    envelope_period: u8,
    envelope_counter: u8,
    envelope_increase: bool,
    dac_enabled: bool,
}

impl SquareChannel {
    fn trigger(&mut self, extra_length_clock: bool) {
        self.enabled = self.dac_enabled;
        if self.length_counter == 0 {
            self.length_counter = 64;
            if self.length_enabled && extra_length_clock {
                self.length_counter -= 1;
            }
        }
        self.timer = square_timer_period(self.freq);
        self.duty_pos = 0;
        self.current_volume = self.initial_volume;
        self.envelope_counter = if self.envelope_period == 0 {
            8
        } else {
            self.envelope_period
        };
    }

    fn apply_envelope_write(&mut self, value: u8) {
        self.initial_volume = (value >> 4) & 0x0F;
        self.current_volume = self.initial_volume;
        self.envelope_increase = (value & 0x08) != 0;
        self.envelope_period = value & 0x07;
        self.dac_enabled = (value & 0xF8) != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    fn tick_timer(&mut self, cycles: i32) {
        if !self.enabled {
            return;
        }

        self.timer -= cycles;
        while self.timer <= 0 {
            self.timer += square_timer_period(self.freq);
            self.duty_pos = (self.duty_pos + 1) & 7;
        }
    }

    fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn clock_envelope(&mut self) {
        if !self.enabled || self.envelope_period == 0 {
            return;
        }

        if self.envelope_counter > 0 {
            self.envelope_counter -= 1;
        }

        if self.envelope_counter == 0 {
            self.envelope_counter = self.envelope_period;
            if self.envelope_increase && self.current_volume < 15 {
                self.current_volume += 1;
            } else if !self.envelope_increase && self.current_volume > 0 {
                self.current_volume -= 1;
            }
        }
    }

    fn output(self) -> i16 {
        if !self.enabled {
            return 0;
        }

        let bit = DUTY_TABLE[usize::from(self.duty)][usize::from(self.duty_pos)];
        if bit == 0 {
            -i16::from(self.current_volume)
        } else {
            i16::from(self.current_volume)
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
struct WaveChannel {
    enabled: bool,
    length_counter: u16,
    length_enabled: bool,
    timer: i32,
    freq: u16,
    position: u8,
    volume_code: u8,
    dac_enabled: bool,
}

impl WaveChannel {
    fn trigger(&mut self, extra_length_clock: bool) {
        self.enabled = self.dac_enabled;
        if self.length_counter == 0 {
            self.length_counter = 256;
            if self.length_enabled && extra_length_clock {
                self.length_counter -= 1;
            }
        }
        self.timer = wave_timer_period(self.freq);
        self.position = 0;
    }

    fn tick_timer(&mut self, cycles: i32) {
        if !self.enabled {
            return;
        }

        self.timer -= cycles;
        while self.timer <= 0 {
            self.timer += wave_timer_period(self.freq);
            self.position = (self.position + 1) & 31;
        }
    }

    fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn output(self, io: &[u8; 0x80]) -> i16 {
        if !self.enabled || !self.dac_enabled {
            return 0;
        }

        let sample_byte = io[0x30 + usize::from(self.position / 2)];
        let sample = if (self.position & 1) == 0 {
            sample_byte >> 4
        } else {
            sample_byte & 0x0F
        };
        let centered = i16::from(sample) - 8;

        match self.volume_code {
            0 => 0,
            1 => centered,
            2 => centered / 2,
            _ => centered / 4,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct NoiseChannel {
    enabled: bool,
    length_counter: u16,
    length_enabled: bool,
    initial_volume: u8,
    current_volume: u8,
    envelope_period: u8,
    envelope_counter: u8,
    envelope_increase: bool,
    lfsr: u16,
    timer: i32,
    divisor_code: u8,
    clock_shift: u8,
    width_mode: bool,
    dac_enabled: bool,
}

impl Default for NoiseChannel {
    fn default() -> Self {
        Self {
            enabled: false,
            length_counter: 0,
            length_enabled: false,
            initial_volume: 0,
            current_volume: 0,
            envelope_period: 0,
            envelope_counter: 0,
            envelope_increase: false,
            lfsr: 0x7FFF,
            timer: 0,
            divisor_code: 0,
            clock_shift: 0,
            width_mode: false,
            dac_enabled: false,
        }
    }
}

impl NoiseChannel {
    fn trigger(&mut self, extra_length_clock: bool) {
        self.enabled = self.dac_enabled;
        if self.length_counter == 0 {
            self.length_counter = 64;
            if self.length_enabled && extra_length_clock {
                self.length_counter -= 1;
            }
        }
        self.lfsr = 0x7FFF;
        self.current_volume = self.initial_volume;
        self.envelope_counter = if self.envelope_period == 0 {
            8
        } else {
            self.envelope_period
        };
        self.timer = noise_timer_period(self.divisor_code, self.clock_shift);
    }

    fn apply_envelope_write(&mut self, value: u8) {
        self.initial_volume = (value >> 4) & 0x0F;
        self.current_volume = self.initial_volume;
        self.envelope_increase = (value & 0x08) != 0;
        self.envelope_period = value & 0x07;
        self.dac_enabled = (value & 0xF8) != 0;
        if !self.dac_enabled {
            self.enabled = false;
        }
    }

    fn tick_timer(&mut self, cycles: i32) {
        if !self.enabled {
            return;
        }

        self.timer -= cycles;
        while self.timer <= 0 {
            self.timer += noise_timer_period(self.divisor_code, self.clock_shift);
            let xor = (self.lfsr & 1) ^ ((self.lfsr >> 1) & 1);
            self.lfsr >>= 1;
            self.lfsr |= xor << 14;
            if self.width_mode {
                self.lfsr = (self.lfsr & !(1 << 6)) | (xor << 6);
            }
        }
    }

    fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn clock_envelope(&mut self) {
        if !self.enabled || self.envelope_period == 0 {
            return;
        }

        if self.envelope_counter > 0 {
            self.envelope_counter -= 1;
        }

        if self.envelope_counter == 0 {
            self.envelope_counter = self.envelope_period;
            if self.envelope_increase && self.current_volume < 15 {
                self.current_volume += 1;
            } else if !self.envelope_increase && self.current_volume > 0 {
                self.current_volume -= 1;
            }
        }
    }

    fn output(self) -> i16 {
        if !self.enabled {
            0
        } else if (self.lfsr & 1) == 1 {
            -i16::from(self.current_volume)
        } else {
            i16::from(self.current_volume)
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApuChannelDiagnostics {
    pub sample_count: u64,
    pub sample_sum: i64,
    pub sample_abs_sum: u64,
    pub peak_abs_sample: u16,
    pub non_zero_samples: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApuDiagnostics {
    pub total_samples: u64,
    pub clipped_samples: u64,
    pub sample_sum: i64,
    pub sample_abs_sum: u64,
    pub min_sample: i16,
    pub max_sample: i16,
    pub peak_abs_sample: u16,
    pub frame_sequencer_steps: u64,
    pub sample_hash: u64,
    pub channel_stats: Option<[ApuChannelDiagnostics; 4]>,
}

impl Default for ApuDiagnostics {
    fn default() -> Self {
        Self {
            total_samples: 0,
            clipped_samples: 0,
            sample_sum: 0,
            sample_abs_sum: 0,
            min_sample: 0,
            max_sample: 0,
            peak_abs_sample: 0,
            frame_sequencer_steps: 0,
            sample_hash: FNV64_OFFSET_BASIS,
            channel_stats: None,
        }
    }
}

impl ApuDiagnostics {
    #[must_use]
    pub fn clipping_ppm(self) -> u64 {
        if self.total_samples == 0 {
            return 0;
        }
        let scaled =
            (u128::from(self.clipped_samples) * 1_000_000u128) / u128::from(self.total_samples);
        match u64::try_from(scaled) {
            Ok(value) => value,
            Err(_) => u64::MAX,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApuDiagnosticsState {
    include_channel_stats: bool,
    total_samples: u64,
    clipped_samples: u64,
    sample_sum: i64,
    sample_abs_sum: u64,
    min_sample: i16,
    max_sample: i16,
    peak_abs_sample: u16,
    frame_sequencer_steps: u64,
    sample_hash: u64,
    channel_stats: [ApuChannelDiagnostics; 4],
}

impl ApuDiagnosticsState {
    fn new(include_channel_stats: bool) -> Self {
        Self {
            include_channel_stats,
            total_samples: 0,
            clipped_samples: 0,
            sample_sum: 0,
            sample_abs_sum: 0,
            min_sample: 0,
            max_sample: 0,
            peak_abs_sample: 0,
            frame_sequencer_steps: 0,
            sample_hash: FNV64_OFFSET_BASIS,
            channel_stats: [ApuChannelDiagnostics::default(); 4],
        }
    }

    fn observe_frame_sequencer_steps(&mut self, steps: u8) {
        self.frame_sequencer_steps = self.frame_sequencer_steps.wrapping_add(u64::from(steps));
    }

    fn observe_channels(&mut self, channels: [i16; 4]) {
        if !self.include_channel_stats {
            return;
        }

        for (stats, sample) in self.channel_stats.iter_mut().zip(channels) {
            stats.sample_count = stats.sample_count.wrapping_add(1);
            stats.sample_sum = stats.sample_sum.wrapping_add(i64::from(sample));

            let abs = abs_sample_as_u16(sample);
            stats.sample_abs_sum = stats.sample_abs_sum.wrapping_add(u64::from(abs));
            stats.peak_abs_sample = stats.peak_abs_sample.max(abs);
            if sample != 0 {
                stats.non_zero_samples = stats.non_zero_samples.wrapping_add(1);
            }
        }
    }

    fn observe_stereo_output(&mut self, left: i16, right: i16, clipped_sides: u8) {
        self.clipped_samples = self.clipped_samples.wrapping_add(u64::from(clipped_sides));
        self.observe_sample(left);
        self.observe_sample(right);
    }

    fn observe_sample(&mut self, sample: i16) {
        self.total_samples = self.total_samples.wrapping_add(1);
        self.sample_sum = self.sample_sum.wrapping_add(i64::from(sample));

        let abs = abs_sample_as_u16(sample);
        self.sample_abs_sum = self.sample_abs_sum.wrapping_add(u64::from(abs));
        self.peak_abs_sample = self.peak_abs_sample.max(abs);

        if self.total_samples == 1 {
            self.min_sample = sample;
            self.max_sample = sample;
        } else {
            self.min_sample = self.min_sample.min(sample);
            self.max_sample = self.max_sample.max(sample);
        }

        for byte in sample.to_le_bytes() {
            self.sample_hash ^= u64::from(byte);
            self.sample_hash = self.sample_hash.wrapping_mul(FNV64_PRIME);
        }
    }

    fn snapshot(&self) -> ApuDiagnostics {
        ApuDiagnostics {
            total_samples: self.total_samples,
            clipped_samples: self.clipped_samples,
            sample_sum: self.sample_sum,
            sample_abs_sum: self.sample_abs_sum,
            min_sample: self.min_sample,
            max_sample: self.max_sample,
            peak_abs_sample: self.peak_abs_sample,
            frame_sequencer_steps: self.frame_sequencer_steps,
            sample_hash: self.sample_hash,
            channel_stats: if self.include_channel_stats {
                Some(self.channel_stats)
            } else {
                None
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
struct DmgHighPassState {
    left_dc_accum: i64,
    right_dc_accum: i64,
}

impl DmgHighPassState {
    fn apply(&mut self, left: i32, right: i32) -> (i32, i32) {
        (
            Self::apply_side(left, &mut self.left_dc_accum),
            Self::apply_side(right, &mut self.right_dc_accum),
        )
    }

    fn apply_side(input: i32, dc_accum: &mut i64) -> i32 {
        *dc_accum = dc_accum
            .wrapping_add(i64::from(input))
            .wrapping_sub(*dc_accum >> HPF_SHIFT);
        let dc_estimate = *dc_accum >> HPF_SHIFT;
        saturating_i32_from_i64(i64::from(input).wrapping_sub(dc_estimate))
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Apu {
    enabled: bool,
    frame_seq: FrameSequencer,
    ch1_sweep: SweepUnit,
    ch1: SquareChannel,
    ch2: SquareChannel,
    ch3: WaveChannel,
    ch4: NoiseChannel,
    sample_accum: u64,
    hpf: DmgHighPassState,
    sample_buffer: Vec<i16>,
    diagnostics: Option<ApuDiagnosticsState>,
}

impl Apu {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_diagnostics_enabled(&mut self, enabled: bool, include_channel_stats: bool) {
        if !enabled {
            self.diagnostics = None;
            return;
        }

        let reuse_existing = self
            .diagnostics
            .as_ref()
            .is_some_and(|state| state.include_channel_stats == include_channel_stats);
        if !reuse_existing {
            self.diagnostics = Some(ApuDiagnosticsState::new(include_channel_stats));
        }
    }

    #[must_use]
    pub fn diagnostics(&self) -> Option<ApuDiagnostics> {
        self.diagnostics.as_ref().map(ApuDiagnosticsState::snapshot)
    }

    pub fn tick(
        &mut self,
        cycles: Cycles,
        io: &mut [u8; 0x80],
        write_mask: u128,
    ) -> Result<(), ApuError> {
        self.apply_power_state(io, write_mask);
        if !self.enabled {
            self.apply_register_writes(io, write_mask);
            io[0x26] = 0x70;
            return Ok(());
        }

        self.apply_register_writes(io, write_mask);

        let step_cycles = i32::try_from(cycles).map_err(|_| ApuError::InvalidState)?;

        let events = self.frame_seq.tick(cycles);
        if events.clock_length {
            self.ch1.clock_length();
            self.ch2.clock_length();
            self.ch3.clock_length();
            self.ch4.clock_length();
        }

        if events.clock_sweep {
            self.ch1_sweep.clock(&mut self.ch1);
        }

        if events.clock_envelope {
            self.ch1.clock_envelope();
            self.ch2.clock_envelope();
            self.ch4.clock_envelope();
        }

        self.ch1.tick_timer(step_cycles);
        self.ch2.tick_timer(step_cycles);
        self.ch3.tick_timer(step_cycles);
        self.ch4.tick_timer(step_cycles);

        self.sample_accum = self
            .sample_accum
            .wrapping_add(u64::from(cycles).wrapping_mul(SAMPLE_RATE));

        if let Some(diagnostics) = self.diagnostics.as_mut() {
            diagnostics.observe_frame_sequencer_steps(events.sequencer_steps);
        }

        while self.sample_accum >= CPU_HZ {
            self.sample_accum -= CPU_HZ;
            let (left, right, channels, clipped_sides) = self.mix(io);
            self.sample_buffer.push(left);
            self.sample_buffer.push(right);

            if let Some(diagnostics) = self.diagnostics.as_mut() {
                diagnostics.observe_channels(channels);
                diagnostics.observe_stereo_output(left, right, clipped_sides);
            }
        }

        self.update_nr52_status(io);
        Ok(())
    }

    #[must_use]
    pub fn take_samples(&mut self) -> Vec<i16> {
        let mut out = Vec::new();
        self.drain_samples_into(&mut out);
        out
    }

    pub fn drain_samples_into(&mut self, out: &mut Vec<i16>) {
        out.append(&mut self.sample_buffer);
    }

    fn apply_power_state(&mut self, io: &mut [u8; 0x80], write_mask: u128) {
        let power_on = (io[0x26] & 0x80) != 0;
        if power_on {
            if !self.enabled {
                self.enabled = true;
                self.frame_seq = FrameSequencer::default();
            }
            return;
        }

        if self.enabled || write_mask_has(write_mask, 0x26) {
            self.power_off(io);
        }
    }

    fn power_off(&mut self, io: &mut [u8; 0x80]) {
        // DMG length counters survive power-off.
        let lengths = [
            self.ch1.length_counter,
            self.ch2.length_counter,
            self.ch3.length_counter,
            self.ch4.length_counter,
        ];
        self.enabled = false;
        self.frame_seq = FrameSequencer::default();
        self.ch1_sweep = SweepUnit::default();
        self.ch1 = SquareChannel::default();
        self.ch2 = SquareChannel::default();
        self.ch3 = WaveChannel::default();
        self.ch4 = NoiseChannel::default();
        self.ch1.length_counter = lengths[0];
        self.ch2.length_counter = lengths[1];
        self.ch3.length_counter = lengths[2];
        self.ch4.length_counter = lengths[3];
        self.sample_accum = 0;
        self.hpf = DmgHighPassState::default();
        self.sample_buffer.clear();

        for register in io.iter_mut().take(0x26).skip(0x10) {
            *register = 0;
        }
        io[0x26] = 0x70;
    }

    #[allow(
        clippy::too_many_lines,
        reason = "APU register matrix is intentionally explicit per hardware register."
    )]
    fn apply_register_writes(&mut self, io: &mut [u8; 0x80], write_mask: u128) {
        if write_mask == 0 {
            return;
        }

        if !self.enabled {
            // DMG still accepts length writes while powered off.
            if write_mask_has(write_mask, 0x11) {
                self.ch1.length_counter = 64 - u16::from(io[0x11] & 0x3F);
            }
            if write_mask_has(write_mask, 0x16) {
                self.ch2.length_counter = 64 - u16::from(io[0x16] & 0x3F);
            }
            if write_mask_has(write_mask, 0x1B) {
                self.ch3.length_counter = 256 - u16::from(io[0x1B]);
            }
            if write_mask_has(write_mask, 0x20) {
                self.ch4.length_counter = 64 - u16::from(io[0x20] & 0x3F);
            }
            for index in 0x10u8..0x26 {
                if write_mask_has(write_mask, index) {
                    io[usize::from(index)] = 0;
                }
            }
            return;
        }

        // The extra clock applies when the next sequencer step won't clock length.
        let extra_length_clock = (self.frame_seq.step & 1) != 0;

        if write_mask_has(write_mask, 0x10) {
            self.ch1_sweep.write_nr10(io[0x10], &mut self.ch1);
        }
        if write_mask_has(write_mask, 0x11) {
            self.ch1.duty = (io[0x11] >> 6) & 0x03;
            self.ch1.length_counter = 64 - u16::from(io[0x11] & 0x3F);
        }
        if write_mask_has(write_mask, 0x12) {
            self.ch1.apply_envelope_write(io[0x12]);
        }
        if write_mask_has(write_mask, 0x13) {
            self.ch1.freq = (self.ch1.freq & 0x0700) | u16::from(io[0x13]);
        }
        if write_mask_has(write_mask, 0x14) {
            let triggering = (io[0x14] & 0x80) != 0;
            write_length_enable(
                &mut self.ch1.length_counter,
                &mut self.ch1.length_enabled,
                &mut self.ch1.enabled,
                (io[0x14] & 0x40) != 0,
                triggering,
                extra_length_clock,
            );
            self.ch1.freq = (self.ch1.freq & 0x00FF) | (u16::from(io[0x14] & 0x07) << 8);
            if triggering {
                self.ch1.trigger(extra_length_clock);
                self.ch1_sweep.trigger(&mut self.ch1);
                io[0x14] &= 0x7F;
            }
        }

        if write_mask_has(write_mask, 0x16) {
            self.ch2.duty = (io[0x16] >> 6) & 0x03;
            self.ch2.length_counter = 64 - u16::from(io[0x16] & 0x3F);
        }
        if write_mask_has(write_mask, 0x17) {
            self.ch2.apply_envelope_write(io[0x17]);
        }
        if write_mask_has(write_mask, 0x18) {
            self.ch2.freq = (self.ch2.freq & 0x0700) | u16::from(io[0x18]);
        }
        if write_mask_has(write_mask, 0x19) {
            let triggering = (io[0x19] & 0x80) != 0;
            write_length_enable(
                &mut self.ch2.length_counter,
                &mut self.ch2.length_enabled,
                &mut self.ch2.enabled,
                (io[0x19] & 0x40) != 0,
                triggering,
                extra_length_clock,
            );
            self.ch2.freq = (self.ch2.freq & 0x00FF) | (u16::from(io[0x19] & 0x07) << 8);
            if triggering {
                self.ch2.trigger(extra_length_clock);
                io[0x19] &= 0x7F;
            }
        }

        if write_mask_has(write_mask, 0x1A) {
            self.ch3.dac_enabled = (io[0x1A] & 0x80) != 0;
            if !self.ch3.dac_enabled {
                self.ch3.enabled = false;
            }
        }
        if write_mask_has(write_mask, 0x1B) {
            self.ch3.length_counter = 256 - u16::from(io[0x1B]);
        }
        if write_mask_has(write_mask, 0x1C) {
            self.ch3.volume_code = (io[0x1C] >> 5) & 0x03;
        }
        if write_mask_has(write_mask, 0x1D) {
            self.ch3.freq = (self.ch3.freq & 0x0700) | u16::from(io[0x1D]);
        }
        if write_mask_has(write_mask, 0x1E) {
            let triggering = (io[0x1E] & 0x80) != 0;
            write_length_enable(
                &mut self.ch3.length_counter,
                &mut self.ch3.length_enabled,
                &mut self.ch3.enabled,
                (io[0x1E] & 0x40) != 0,
                triggering,
                extra_length_clock,
            );
            self.ch3.freq = (self.ch3.freq & 0x00FF) | (u16::from(io[0x1E] & 0x07) << 8);
            if triggering {
                self.ch3.trigger(extra_length_clock);
                io[0x1E] &= 0x7F;
            }
        }

        if write_mask_has(write_mask, 0x20) {
            self.ch4.length_counter = 64 - u16::from(io[0x20] & 0x3F);
        }
        if write_mask_has(write_mask, 0x21) {
            self.ch4.apply_envelope_write(io[0x21]);
        }
        if write_mask_has(write_mask, 0x22) {
            self.ch4.clock_shift = (io[0x22] >> 4) & 0x0F;
            self.ch4.width_mode = (io[0x22] & 0x08) != 0;
            self.ch4.divisor_code = io[0x22] & 0x07;
        }
        if write_mask_has(write_mask, 0x23) {
            let triggering = (io[0x23] & 0x80) != 0;
            write_length_enable(
                &mut self.ch4.length_counter,
                &mut self.ch4.length_enabled,
                &mut self.ch4.enabled,
                (io[0x23] & 0x40) != 0,
                triggering,
                extra_length_clock,
            );
            if triggering {
                self.ch4.trigger(extra_length_clock);
                io[0x23] &= 0x7F;
            }
        }
    }

    fn update_nr52_status(&self, io: &mut [u8; 0x80]) {
        io[0x26] = (io[0x26] & 0x80)
            | 0x70
            | u8::from(self.ch1.enabled)
            | (u8::from(self.ch2.enabled) << 1)
            | (u8::from(self.ch3.enabled) << 2)
            | (u8::from(self.ch4.enabled) << 3);
    }

    fn mix(&mut self, io: &[u8; 0x80]) -> (i16, i16, [i16; 4], u8) {
        let channels = [
            self.ch1.output(),
            self.ch2.output(),
            self.ch3.output(io),
            self.ch4.output(),
        ];
        let nr50 = io[0x24];
        let nr51 = io[0x25];

        let mut left = 0i32;
        let mut right = 0i32;

        for (index, sample) in channels.iter().enumerate() {
            if (nr51 & (1 << index)) != 0 {
                right += i32::from(*sample);
            }
            if (nr51 & (1 << (index + 4))) != 0 {
                left += i32::from(*sample);
            }
        }

        let left_vol = i32::from((nr50 >> 4) & 0x07) + 1;
        let right_vol = i32::from(nr50 & 0x07) + 1;

        left = left * left_vol * MIX_OUTPUT_GAIN;
        right = right * right_vol * MIX_OUTPUT_GAIN;

        let (left, right) = self.hpf.apply(left, right);
        let (left, left_clipped) = i16_from_i32_with_clipping(left);
        let (right, right_clipped) = i16_from_i32_with_clipping(right);
        let clipped_sides = u8::from(left_clipped) + u8::from(right_clipped);
        (left, right, channels, clipped_sides)
    }
}

fn square_timer_period(freq: u16) -> i32 {
    (i32::from(2_048u16.saturating_sub(freq)) * 4).max(4)
}

fn wave_timer_period(freq: u16) -> i32 {
    (i32::from(2_048u16.saturating_sub(freq)) * 2).max(2)
}

fn noise_timer_period(divisor_code: u8, clock_shift: u8) -> i32 {
    let divisor: u32 = match divisor_code {
        0 => 8,
        1 => 16,
        2 => 32,
        3 => 48,
        4 => 64,
        5 => 80,
        6 => 96,
        _ => 112,
    };
    let period = divisor << clock_shift;
    match i32::try_from(period) {
        Ok(value) => value.max(1),
        Err(_) => i32::MAX,
    }
}

fn i16_from_i32_with_clipping(value: i32) -> (i16, bool) {
    match i16::try_from(value) {
        Ok(sample) => (sample, false),
        Err(_) if value < 0 => (i16::MIN, true),
        Err(_) => (i16::MAX, true),
    }
}

fn abs_sample_as_u16(sample: i16) -> u16 {
    let abs = i32::from(sample).unsigned_abs();
    match u16::try_from(abs) {
        Ok(value) => value,
        Err(_) => u16::MAX,
    }
}

fn saturating_i32_from_i64(value: i64) -> i32 {
    match i32::try_from(value) {
        Ok(as_i32) => as_i32,
        Err(_) if value < i64::from(i32::MIN) => i32::MIN,
        Err(_) => i32::MAX,
    }
}

/// Applies the length-enable bit of an `NRx4` write. Enabling length while the
/// next frame sequencer step won't clock it clocks the counter once now.
fn write_length_enable(
    length_counter: &mut u16,
    length_enabled: &mut bool,
    channel_enabled: &mut bool,
    enable: bool,
    triggering: bool,
    extra_length_clock: bool,
) {
    let was_enabled = *length_enabled;
    *length_enabled = enable;
    if extra_length_clock && !was_enabled && enable && *length_counter > 0 {
        *length_counter -= 1;
        if *length_counter == 0 && !triggering {
            *channel_enabled = false;
        }
    }
}

fn write_mask_has(mask: u128, index: u8) -> bool {
    (mask & (1u128 << index)) != 0
}

#[cfg(test)]
mod tests {
    use super::Apu;

    #[test]
    fn mixer_preserves_headroom_when_all_channels_peak() {
        let mut apu = Apu::new();
        apu.enabled = true;

        apu.ch1.enabled = true;
        apu.ch1.current_volume = 15;
        apu.ch1.duty = 2;
        apu.ch1.duty_pos = 0;

        apu.ch2.enabled = true;
        apu.ch2.current_volume = 15;
        apu.ch2.duty = 2;
        apu.ch2.duty_pos = 0;

        apu.ch3.enabled = true;
        apu.ch3.volume_code = 1;
        apu.ch3.position = 0;
        apu.ch3.dac_enabled = true;

        apu.ch4.enabled = true;
        apu.ch4.current_volume = 15;
        apu.ch4.lfsr = 0;

        let mut io = [0u8; 0x80];
        io[0x24] = 0x77;
        io[0x25] = 0xFF;
        io[0x30] = 0xFF;

        let (left, right, _, clipped_sides) = apu.mix(&io);
        assert_eq!(left, 19_959);
        assert_eq!(right, 19_959);
        assert_eq!(clipped_sides, 0);
    }

    #[test]
    fn diagnostics_are_opt_in_and_report_channel_stats() {
        let mut apu = Apu::new();
        let mut io = [0u8; 0x80];

        io[0x26] = 0x80;
        io[0x24] = 0x77;
        io[0x25] = 0x11;
        io[0x11] = 0xC0;
        io[0x12] = 0xF3;
        io[0x13] = 0x70;
        io[0x14] = 0x80;

        let write_mask = (1u128 << 0x11) | (1u128 << 0x12) | (1u128 << 0x13) | (1u128 << 0x14);
        apu.tick(20_000, &mut io, write_mask)
            .expect("tick should succeed");
        assert!(apu.diagnostics().is_none());

        apu.set_diagnostics_enabled(true, true);
        io[0x14] = 0x80;
        apu.tick(20_000, &mut io, 1u128 << 0x14)
            .expect("tick should succeed");

        let diagnostics = apu.diagnostics().expect("diagnostics should be available");
        assert!(diagnostics.total_samples > 0);
        assert!(diagnostics.frame_sequencer_steps > 0);
        assert!(diagnostics.channel_stats.is_some());
    }
}
