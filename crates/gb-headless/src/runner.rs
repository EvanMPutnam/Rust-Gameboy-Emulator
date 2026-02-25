use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use gb_core::{CartridgeSave, GameBoy, GbButton, GbConfig, GbError};

use crate::report::{AudioDiagnosticsReport, RunReport, hash_bytes, hash_samples};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunBudget {
    Cycles(u32),
    Frames(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerConfig {
    pub rom_path: PathBuf,
    pub boot_rom_path: Option<PathBuf>,
    pub budget: RunBudget,
    pub input_script_path: Option<PathBuf>,
    pub save_in_path: Option<PathBuf>,
    pub save_out_path: Option<PathBuf>,
    pub audio_diagnostics: bool,
    pub audio_channel_diagnostics: bool,
}

#[derive(Debug)]
pub enum RunnerError {
    Io(std::io::Error),
    Core(GbError),
    InvalidBootRomLength(usize),
    InputScriptParse(String),
    FrameBudgetTimeout,
}

impl Display for RunnerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Core(err) => write!(f, "core error: {err}"),
            Self::InvalidBootRomLength(len) => {
                write!(f, "boot rom must contain exactly 256 bytes, got {len}")
            }
            Self::InputScriptParse(msg) => write!(f, "input script parse error: {msg}"),
            Self::FrameBudgetTimeout => write!(f, "frame budget could not be reached in time"),
        }
    }
}

impl Error for RunnerError {}

impl From<std::io::Error> for RunnerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<GbError> for RunnerError {
    fn from(value: GbError) -> Self {
        Self::Core(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InputEvent {
    cycle: u64,
    button: GbButton,
    pressed: bool,
}

pub fn run(config: &RunnerConfig) -> Result<RunReport, RunnerError> {
    let rom = fs::read(&config.rom_path)?;
    let mut gb_config = GbConfig::default();

    if let Some(path) = &config.boot_rom_path {
        let bytes = fs::read(path)?;
        if bytes.len() != 256 {
            return Err(RunnerError::InvalidBootRomLength(bytes.len()));
        }

        let mut boot = [0u8; 256];
        boot.copy_from_slice(&bytes);
        gb_config.use_boot_rom = true;
        gb_config.force_post_boot_state = false;
        gb_config.boot_rom = Some(boot);
    }

    let mut gameboy = GameBoy::new(gb_config, rom)?;
    if config.audio_diagnostics {
        gameboy.set_apu_diagnostics_enabled(true, config.audio_channel_diagnostics);
    }

    if let Some(path) = &config.save_in_path {
        let save_ram = fs::read(path)?;
        if !save_ram.is_empty() {
            let save = CartridgeSave {
                ram: Some(save_ram),
                rtc: None,
            };
            gameboy.import_save(&save)?;
        }
    }

    let mut input_events = if let Some(path) = &config.input_script_path {
        parse_input_script(path)?
    } else {
        Vec::new()
    };
    input_events.sort_by_key(|event| event.cycle);
    let mut event_index = 0usize;

    let mut cycles_executed = 0u64;
    let mut frames_produced = 0u64;
    let mut frame_hash = None;
    let mut audio_hash = None;
    let mut audio_samples = Vec::new();
    let frame_timeout = match config.budget {
        RunBudget::Frames(frames) => u64::from(frames).saturating_mul(90_000),
        RunBudget::Cycles(_) => u64::MAX,
    };

    loop {
        let done = match config.budget {
            RunBudget::Cycles(target) => cycles_executed >= u64::from(target),
            RunBudget::Frames(target) => frames_produced >= u64::from(target),
        };
        if done {
            break;
        }

        if matches!(config.budget, RunBudget::Frames(_)) && cycles_executed > frame_timeout {
            return Err(RunnerError::FrameBudgetTimeout);
        }

        while event_index < input_events.len() && input_events[event_index].cycle <= cycles_executed
        {
            let event = input_events[event_index];
            gameboy.set_button(event.button, event.pressed)?;
            event_index += 1;
        }

        cycles_executed = cycles_executed.wrapping_add(u64::from(gameboy.step()?));

        if let Some(frame) = gameboy.take_frame() {
            frames_produced = frames_produced.wrapping_add(1);
            frame_hash = Some(hash_bytes(&frame));
        }

        audio_samples.clear();
        gameboy.drain_audio_samples_into(&mut audio_samples);
        if !audio_samples.is_empty() {
            let current = hash_samples(&audio_samples);
            audio_hash = Some(audio_hash.map_or(current, |prior: u64| prior ^ current));
        }
    }

    if let Some(path) = &config.save_out_path {
        let save = gameboy.export_save()?;
        let payload = save.ram.unwrap_or_default();
        fs::write(path, payload)?;
    }

    Ok(RunReport {
        cycles_executed,
        frames_produced,
        frame_hash,
        audio_hash,
        audio_diagnostics: if config.audio_diagnostics {
            gameboy.apu_diagnostics().map(AudioDiagnosticsReport::from)
        } else {
            None
        },
    })
}

fn parse_input_script(path: &Path) -> Result<Vec<InputEvent>, RunnerError> {
    let contents = fs::read_to_string(path)?;
    let mut events = Vec::new();

    for (line_number, raw) in contents.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split(',').map(str::trim).collect();
        if parts.len() != 3 {
            return Err(RunnerError::InputScriptParse(format!(
                "line {} must contain 3 comma-separated fields",
                line_number + 1
            )));
        }

        let cycle = parts[0].parse::<u64>().map_err(|_| {
            RunnerError::InputScriptParse(format!(
                "line {} has invalid cycle value '{}'",
                line_number + 1,
                parts[0]
            ))
        })?;
        let button = parse_button(parts[1]).ok_or_else(|| {
            RunnerError::InputScriptParse(format!(
                "line {} has unknown button '{}'",
                line_number + 1,
                parts[1]
            ))
        })?;
        let pressed = parse_pressed(parts[2]).ok_or_else(|| {
            RunnerError::InputScriptParse(format!(
                "line {} has invalid state '{}'",
                line_number + 1,
                parts[2]
            ))
        })?;

        events.push(InputEvent {
            cycle,
            button,
            pressed,
        });
    }

    Ok(events)
}

fn parse_button(value: &str) -> Option<GbButton> {
    match value.to_ascii_lowercase().as_str() {
        "right" => Some(GbButton::Right),
        "left" => Some(GbButton::Left),
        "up" => Some(GbButton::Up),
        "down" => Some(GbButton::Down),
        "a" => Some(GbButton::A),
        "b" => Some(GbButton::B),
        "select" => Some(GbButton::Select),
        "start" => Some(GbButton::Start),
        _ => None,
    }
}

fn parse_pressed(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "down" | "press" | "pressed" | "1" | "true" => Some(true),
        "up" | "release" | "released" | "0" | "false" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{RunBudget, RunnerConfig, parse_input_script, run};

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{name}_{nanos}"))
    }

    fn write_test_rom(path: &PathBuf) {
        let mut rom = vec![0u8; 0x8000];
        rom[0x0147] = 0x00;
        rom[0x0148] = 0x00;
        rom[0x0149] = 0x00;
        rom[0x0100] = 0x00;
        std::fs::write(path, rom).expect("write rom");
    }

    #[test]
    fn parses_input_script_lines() {
        let path = temp_path("input_script");
        std::fs::write(&path, "0,right,down\n100,right,up\n").expect("write script");
        let events = parse_input_script(&path).expect("parse script");
        assert_eq!(events.len(), 2);
        std::fs::remove_file(&path).expect("remove script");
    }

    #[test]
    fn runner_produces_deterministic_report_for_fixed_cycles() {
        let rom_path = temp_path("runner_rom");
        write_test_rom(&rom_path);

        let config = RunnerConfig {
            rom_path: rom_path.clone(),
            boot_rom_path: None,
            budget: RunBudget::Cycles(1_024),
            input_script_path: None,
            save_in_path: None,
            save_out_path: None,
            audio_diagnostics: false,
            audio_channel_diagnostics: false,
        };

        let first = run(&config).expect("run first");
        let second = run(&config).expect("run second");
        assert_eq!(first, second);

        std::fs::remove_file(&rom_path).expect("remove rom");
    }
}
