use std::env;
use std::path::PathBuf;

use compat::CompatTier;
use runner::{RunBudget, RunnerConfig, run};

mod compat;
mod report;
mod runner;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match parse_args(&args) {
        Ok(Command::Single(config)) => match run(&config) {
            Ok(report) => {
                println!("{}", report.to_json());
            }
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        },
        Ok(Command::Compat { tier, manifest }) => match compat::runner::run_tier(tier, manifest) {
            Ok(report) => match report.to_json() {
                Ok(json) => println!("{json}"),
                Err(err) => {
                    eprintln!("{err}");
                    std::process::exit(1);
                }
            },
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        },
        Err(err) => {
            eprintln!("{err}");
            eprintln!(
                "usage: --rom <path> [--boot-rom <path>] (--cycles <n> | --frames <n>) [--input-script <path>] [--save-in <path>] [--save-out <path>]"
            );
            eprintln!("   or: --compat-tier <smoke|core|full> [--manifest <path>]");
            std::process::exit(2);
        }
    }
}

#[derive(Debug)]
enum Command {
    Single(RunnerConfig),
    Compat {
        tier: CompatTier,
        manifest: Option<PathBuf>,
    },
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    let mut rom_path = None;
    let mut boot_rom_path = None;
    let mut cycles = None;
    let mut frames = None;
    let mut input_script_path = None;
    let mut save_in_path = None;
    let mut save_out_path = None;
    let mut compat_tier = None;
    let mut compat_manifest = None;

    let mut index = 0usize;
    while index < args.len() {
        let key = &args[index];
        let Some(value) = args.get(index + 1) else {
            return Err(format!("missing value for argument '{key}'"));
        };

        match key.as_str() {
            "--rom" => rom_path = Some(PathBuf::from(value)),
            "--boot-rom" => boot_rom_path = Some(PathBuf::from(value)),
            "--cycles" => {
                cycles = Some(
                    value
                        .parse::<u32>()
                        .map_err(|_| format!("invalid cycle count '{value}'"))?,
                );
            }
            "--frames" => {
                frames = Some(
                    value
                        .parse::<u32>()
                        .map_err(|_| format!("invalid frame count '{value}'"))?,
                );
            }
            "--input-script" => input_script_path = Some(PathBuf::from(value)),
            "--save-in" => save_in_path = Some(PathBuf::from(value)),
            "--save-out" => save_out_path = Some(PathBuf::from(value)),
            "--compat-tier" => {
                compat_tier = Some(CompatTier::from_str(value).map_err(|err| format!("{err}"))?);
            }
            "--manifest" => compat_manifest = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown argument '{key}'")),
        }

        index += 2;
    }

    if let Some(tier) = compat_tier {
        return Ok(Command::Compat {
            tier,
            manifest: compat_manifest,
        });
    }

    let rom_path = rom_path.ok_or_else(|| String::from("missing required --rom argument"))?;
    let budget = match (cycles, frames) {
        (Some(_), Some(_)) => {
            return Err(String::from(
                "provide only one run budget: --cycles or --frames",
            ));
        }
        (Some(value), None) => RunBudget::Cycles(value),
        (None, Some(value)) => RunBudget::Frames(value),
        (None, None) => return Err(String::from("missing run budget: --cycles or --frames")),
    };

    Ok(Command::Single(RunnerConfig {
        rom_path,
        boot_rom_path,
        budget,
        input_script_path,
        save_in_path,
        save_out_path,
    }))
}

#[cfg(test)]
mod tests {
    use super::{Command, parse_args};
    use crate::compat::CompatTier;
    use crate::runner::RunBudget;

    #[test]
    fn parse_args_rejects_missing_rom() {
        let args = vec![String::from("--cycles"), String::from("10")];
        let err = parse_args(&args).expect_err("parse should fail");
        assert!(err.contains("--rom"));
    }

    #[test]
    fn parse_args_accepts_valid_cycle_run() {
        let args = vec![
            String::from("--rom"),
            String::from("game.gb"),
            String::from("--cycles"),
            String::from("100"),
        ];
        let command = parse_args(&args).expect("parse args");

        match command {
            Command::Single(config) => {
                assert_eq!(config.budget, RunBudget::Cycles(100));
            }
            Command::Compat { .. } => panic!("expected single command"),
        }
    }

    #[test]
    fn parse_args_accepts_compat_mode() {
        let args = vec![String::from("--compat-tier"), String::from("smoke")];

        let command = parse_args(&args).expect("parse args");
        match command {
            Command::Compat { tier, .. } => assert_eq!(tier, CompatTier::Smoke),
            Command::Single(_) => panic!("expected compat command"),
        }
    }
}
