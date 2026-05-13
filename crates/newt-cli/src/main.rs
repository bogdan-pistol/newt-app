use anyhow::{Context, Result, bail};
use clap::{ArgGroup, Args, Parser, Subcommand};
use newt_core::{
    config::Config,
    doctor,
    paths::Paths,
    prompt,
    provider::{Provider, RewriteEvent, mock::MockProvider},
    rewrite,
    simulate,
};
use serde_json::json;
use std::io::{Read, Write};

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
    Doctor {
        /// Include end-to-end checks against the mock provider (PRD §8.6).
        #[arg(long)]
        full: bool,
    },
    /// Run a rewrite against a provider, streaming the result.
    Rewrite(RewriteArgs),
    /// Simulate a selection capture and run the rewrite pipeline as if the
    /// text had come from the OS clipboard. Writes the result to the replace
    /// buffer (the Phase 1 stand-in for paste-back). PRD §8.2.
    SimulateSelection(RewriteArgs),
    /// Inspect or write the replace buffer (the paste-back test target).
    /// With no `--text`, prints the current buffer. PRD §8.2.
    SimulateReplace(SimulateReplaceArgs),
}

#[derive(Args)]
#[command(group(
    ArgGroup::new("input_src").required(true).args(["text", "input"])
))]
struct RewriteArgs {
    /// Prompt id (filename stem under the prompts directory).
    #[arg(short, long, value_name = "ID")]
    prompt: String,

    /// Inline text to rewrite. Mutually exclusive with --input.
    #[arg(long)]
    text: Option<String>,

    /// Read selection from a file. Use `-` for stdin. Mutually exclusive with --text.
    #[arg(short, long, value_name = "PATH")]
    input: Option<String>,

    /// Provider to use. Phase 1 only ships the deterministic mock.
    #[arg(long, default_value = "mock")]
    provider: String,
}

#[derive(Args)]
struct SimulateReplaceArgs {
    /// Text to write to the replace buffer. If omitted, the current buffer
    /// contents are printed without modification.
    #[arg(long)]
    text: Option<String>,
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
    // Idempotent: seeds defaults on first run, no-op on subsequent invocations.
    // Centralizing this here means every subcommand sees a consistent prompts dir.
    prompt::seed_defaults_if_empty(&paths)?;

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
        Some(Command::Doctor { full }) => run_doctor(&paths, full, cli.json),
        Some(Command::Rewrite(args)) => run_rewrite(&paths, args, cli.json),
        Some(Command::SimulateSelection(args)) => run_simulate_selection(&paths, args, cli.json),
        Some(Command::SimulateReplace(args)) => run_simulate_replace(&paths, args, cli.json),
    }
}

fn run_prompts(paths: &Paths, action: PromptsAction, json: bool) -> Result<()> {
    match action {
        PromptsAction::List => {
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

fn run_doctor(paths: &Paths, full: bool, json: bool) -> Result<()> {
    let report = if full {
        doctor::run_full(paths).context("running full health checks")?
    } else {
        doctor::run(paths).context("running health checks")?
    };

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

fn run_rewrite(paths: &Paths, args: RewriteArgs, json: bool) -> Result<()> {
    let selection = read_selection(&args)?;
    let provider = build_provider(&args.provider, &selection)?;

    stream_pipeline(json, |emit| {
        rewrite::run(paths, &args.prompt, &selection, provider.as_ref(), emit)
    })
}

fn run_simulate_selection(paths: &Paths, args: RewriteArgs, json: bool) -> Result<()> {
    let selection = read_selection(&args)?;
    let provider = build_provider(&args.provider, &selection)?;

    stream_pipeline(json, |emit| {
        // simulate::selection returns the concatenated text; we discard it
        // here because the test harness reads the replace buffer instead.
        simulate::selection(paths, &args.prompt, &selection, provider.as_ref(), emit).map(|_| ())
    })
}

fn run_simulate_replace(paths: &Paths, args: SimulateReplaceArgs, json: bool) -> Result<()> {
    if let Some(text) = &args.text {
        simulate::write_replace_buffer(paths, text)?;
    }
    let buffer = simulate::read_replace_buffer(paths)?;
    if json {
        println!("{}", json!({ "buffer": buffer }));
    } else {
        print!("{buffer}");
        if !buffer.ends_with('\n') {
            println!();
        }
    }
    Ok(())
}

/// Run a streaming pipeline (`rewrite::run`, `simulate::selection`, …) and
/// render its events. In `--json` mode every event is one NDJSON line; in
/// human mode only `Token` events print, joined into a stream of text with
/// a single trailing newline.
fn stream_pipeline<F>(json: bool, run: F) -> Result<()>
where
    F: FnOnce(&mut dyn FnMut(RewriteEvent)) -> Result<()>,
{
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut wrote_token = false;

    let result = run(&mut |event| {
        if json {
            let _ = serde_json::to_writer(&mut out, &event);
            let _ = writeln!(out);
        } else if let RewriteEvent::Token { text } = &event {
            let _ = write!(out, "{text}");
            let _ = out.flush();
            wrote_token = true;
        }
    });

    if !json && wrote_token {
        let _ = writeln!(out);
    }

    if let Err(e) = result {
        if json {
            let payload = json!({ "type": "error", "message": format!("{e:#}") });
            let _ = serde_json::to_writer(&mut out, &payload);
            let _ = writeln!(out);
        }
        return Err(e);
    }
    Ok(())
}

fn read_selection(args: &RewriteArgs) -> Result<String> {
    if let Some(text) = &args.text {
        return Ok(text.clone());
    }
    let path = args.input.as_deref().expect("clap group enforces one of text/input");
    if path == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("reading stdin")?;
        return Ok(buf);
    }
    std::fs::read_to_string(path).with_context(|| format!("reading {path}"))
}

fn build_provider(name: &str, selection: &str) -> Result<Box<dyn Provider>> {
    match name {
        "mock" => Ok(Box::new(MockProvider::echo(format!("[mock] {selection}")))),
        other => bail!("unknown provider: `{other}` (Phase 1 supports: mock)"),
    }
}
