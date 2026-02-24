mod app;
mod audio;
mod error;
mod input;
mod platform;
mod runloop;
mod video;

use std::env;
use std::path::PathBuf;

use error::DesktopError;
use platform::{LaunchOptions, run};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let options = match parse_args(&args) {
        Ok(options) => options,
        Err(err) => {
            eprintln!("{err}");
            eprintln!(
                "usage: --rom <path> [--boot-rom <path>] [--save-in <path>] [--save-out <path>] [--max-cycles <n>] [--mute] [--audio-diagnostics] [--video-diagnostics]"
            );
            std::process::exit(2);
        }
    };

    if let Err(err) = run(options) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn parse_args(args: &[String]) -> Result<LaunchOptions, DesktopError> {
    let mut rom_path = None;
    let mut boot_rom_path = None;
    let mut save_in_path = None;
    let mut save_out_path = None;
    let mut max_cycles = None;
    let mut mute = false;
    let mut audio_diagnostics = false;
    let mut video_diagnostics = false;

    let mut index = 0usize;
    while index < args.len() {
        let key = &args[index];
        match key.as_str() {
            "--mute" => {
                mute = true;
                index += 1;
            }
            "--audio-diagnostics" => {
                audio_diagnostics = true;
                index += 1;
            }
            "--video-diagnostics" => {
                video_diagnostics = true;
                index += 1;
            }
            "--rom" | "--boot-rom" | "--save-in" | "--save-out" | "--max-cycles" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(DesktopError::Argument(format!(
                        "missing value for argument '{key}'"
                    )));
                };
                match key.as_str() {
                    "--rom" => rom_path = Some(PathBuf::from(value)),
                    "--boot-rom" => boot_rom_path = Some(PathBuf::from(value)),
                    "--save-in" => save_in_path = Some(PathBuf::from(value)),
                    "--save-out" => save_out_path = Some(PathBuf::from(value)),
                    "--max-cycles" => {
                        max_cycles = Some(value.parse::<u64>().map_err(|_| {
                            DesktopError::Argument(format!("invalid max cycle count '{value}'"))
                        })?);
                    }
                    _ => {}
                }
                index += 2;
            }
            _ => {
                return Err(DesktopError::Argument(format!("unknown argument '{key}'")));
            }
        }
    }

    let rom_path = rom_path
        .ok_or_else(|| DesktopError::Argument(String::from("missing required --rom argument")))?;

    Ok(LaunchOptions {
        rom_path,
        boot_rom_path,
        save_in_path,
        save_out_path,
        max_cycles,
        mute,
        audio_diagnostics,
        video_diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_args;

    #[test]
    fn parse_args_requires_rom() {
        let args = vec![String::from("--mute")];
        let error = parse_args(&args).expect_err("expected missing rom error");
        assert!(error.to_string().contains("--rom"));
    }

    #[test]
    fn parse_args_accepts_optional_flags() {
        let args = vec![
            String::from("--rom"),
            String::from("test.gb"),
            String::from("--boot-rom"),
            String::from("boot.bin"),
            String::from("--save-in"),
            String::from("input.sav"),
            String::from("--save-out"),
            String::from("output.sav"),
            String::from("--max-cycles"),
            String::from("4096"),
            String::from("--mute"),
            String::from("--audio-diagnostics"),
            String::from("--video-diagnostics"),
        ];

        let options = parse_args(&args).expect("parse args");
        assert_eq!(options.max_cycles, Some(4096));
        assert!(options.mute);
        assert!(options.audio_diagnostics);
        assert!(options.video_diagnostics);
        assert!(options.boot_rom_path.is_some());
        assert!(options.save_in_path.is_some());
        assert!(options.save_out_path.is_some());
    }
}
