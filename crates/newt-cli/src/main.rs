use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use newt_core::{config::Config, doctor, paths::Paths, prompt};
use serde_json::json;

/// Newt — system-wide AI text rewriter.
#[derive(Parser)]
#[command(name = "newt", version, about, long_about = None)]
struct Cli {
    /// Emit machine-readable JSON instead of human output.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Manage prompts.
    Prompts {
        #[command(subcommand)]
        action: PromptsAction,
    },
    /// Get and set configuration values.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Run health checks.
    Doctor,
}

#[derive(Subcommand)]
enum PromptsAction {
    /// List available prompts.
    List,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Get a config value by key.
    Get { key: String },
    /// Set a config value by key.
    Set { key: String, value: String },
    /// Unset a config value.
    Unset { key: String },
    /// List all config keys and values.
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = Paths::from_env()?;

    match cli.command {
        None => {
            // Bare `newt` with no subcommand — print version, same as before.
            println!(
                "newt {} (core {})",
                env!("CARGO_PKG_VERSION"),
                newt_core::version()
            );
            Ok(())
        }
        Some(Command::Prompts { action }) => run_prompts(&paths, action, cli.json),
        Some(Command::Config { action }) => run_config(&paths, action, cli.json),
        Some(Command::Doctor) => run_doctor(&paths, cli.json),
    }
}

fn run_prompts(paths: &Paths, action: PromptsAction, json: bool) -> Result<()> {
    match action {
        PromptsAction::List => {
            // Seed defaults on first use so a fresh install has something to show.
            prompt::seed_defaults_if_empty(paths)?;
            let prompts = prompt::list(paths)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&prompts)?);
            } else if prompts.is_empty() {
                println!("(no prompts)");
            } else {
                for p in &prompts {
                    let emoji = p.emoji.as_deref().unwrap_or("  ");
                    println!("{emoji}  {:<24} {}", p.id, p.description);
                }
            }
            Ok(())
        }
    }
}

fn run_config(paths: &Paths, action: ConfigAction, json: bool) -> Result<()> {
    match action {
        ConfigAction::Get { key } => {
            let cfg = Config::load(paths)?;
            match cfg.get(&key) {
                Some(v) => {
                    if json {
                        println!("{}", json!({ "key": key, "value": v }));
                    } else {
                        println!("{v}");
                    }
                    Ok(())
                }
                None => {
                    if json {
                        println!("{}", json!({ "key": key, "value": null }));
                        Ok(())
                    } else {
                        bail!("no value set for `{key}`");
                    }
                }
            }
        }
        ConfigAction::Set { key, value } => {
            let mut cfg = Config::load(paths)?;
            cfg.set(&key, &value);
            cfg.save(paths)?;
            if json {
                println!("{}", json!({ "key": key, "value": value, "set": true }));
            } else {
                println!("set {key} = {value}");
            }
            Ok(())
        }
        ConfigAction::Unset { key } => {
            let mut cfg = Config::load(paths)?;
            let removed = cfg.unset(&key);
            cfg.save(paths)?;
            if json {
                println!("{}", json!({ "key": key, "removed": removed }));
            } else if removed {
                println!("unset {key}");
            } else {
                println!("(no value was set for `{key}`)");
            }
            Ok(())
        }
        ConfigAction::List => {
            let cfg = Config::load(paths)?;
            if json {
                let map: std::collections::BTreeMap<&str, &str> = cfg.iter().collect();
                println!("{}", serde_json::to_string_pretty(&map)?);
            } else {
                for (k, v) in cfg.iter() {
                    println!("{k} = {v}");
                }
            }
            Ok(())
        }
    }
}

fn run_doctor(paths: &Paths, json: bool) -> Result<()> {
    let report = doctor::run(paths).context("running health checks")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        for c in &report.checks {
            let mark = match c.status {
                doctor::Status::Pass => "✓",
                doctor::Status::Warn => "!",
                doctor::Status::Fail => "✗",
            };
            println!("{mark}  {:<22} {}", c.name, c.message);
        }
    }

    if !report.ok() {
        std::process::exit(1);
    }
    Ok(())
}
