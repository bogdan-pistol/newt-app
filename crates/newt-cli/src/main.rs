use anyhow::{Context, Result, anyhow, bail};
use clap::{ArgGroup, Args, Parser, Subcommand};
use newt_core::{
    config::Config,
    doctor, keychain,
    paths::Paths,
    prompt,
    provider::{
        Provider, RewriteEvent, RewriteRequest, anthropic::AnthropicProvider, mock::MockProvider,
        openai::OpenAiProvider,
    },
    rewrite, simulate,
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
    /// Manage LLM provider keys and connectivity.
    Providers {
        #[command(subcommand)]
        action: ProvidersAction,
    },
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

#[derive(Subcommand)]
enum ProvidersAction {
    /// List known providers and whether each has a key in Keychain.
    List,
    /// Store an API key for a provider in macOS Keychain. Prompts hidden by
    /// default; pass `--from-stdin` to read the key from stdin (for piping).
    SetKey {
        /// Provider name: `openai` or `anthropic`.
        provider: String,
        /// Read the key from stdin instead of prompting interactively.
        #[arg(long)]
        from_stdin: bool,
    },
    /// Print the stored key for a provider, masked unless `--reveal` is set.
    GetKey {
        provider: String,
        /// Print the full key in clear text.
        #[arg(long)]
        reveal: bool,
    },
    /// Remove a provider's key from Keychain.
    RemoveKey { provider: String },
    /// Run a tiny live rewrite against a provider to verify connectivity.
    /// Costs a fraction of a cent in tokens.
    Test { provider: String },
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
        Some(Command::Providers { action }) => run_providers(action, cli.json),
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
    let path = args
        .input
        .as_deref()
        .expect("clap group enforces one of text/input");
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
        "openai" => {
            let key = require_key("openai")?;
            Ok(Box::new(OpenAiProvider::new(key)))
        }
        "anthropic" => {
            let key = require_key("anthropic")?;
            Ok(Box::new(AnthropicProvider::new(key)))
        }
        other => bail!("unknown provider: `{other}` (supported: mock, openai, anthropic)"),
    }
}

fn require_key(provider: &str) -> Result<String> {
    keychain::get_key(provider)?.ok_or_else(|| {
        anyhow!("no API key for `{provider}` in Keychain — run: newt providers set-key {provider}")
    })
}

fn run_providers(action: ProvidersAction, json: bool) -> Result<()> {
    match action {
        ProvidersAction::List => providers_list(json),
        ProvidersAction::SetKey {
            provider,
            from_stdin,
        } => providers_set_key(&provider, from_stdin, json),
        ProvidersAction::GetKey { provider, reveal } => providers_get_key(&provider, reveal, json),
        ProvidersAction::RemoveKey { provider } => providers_remove_key(&provider, json),
        ProvidersAction::Test { provider } => providers_test(&provider, json),
    }
}

fn providers_list(json: bool) -> Result<()> {
    // Stable order; mock first because it never needs a key.
    let providers = [
        ("mock", false, "deterministic test provider"),
        ("openai", true, "OpenAI chat completions"),
        ("anthropic", true, "Anthropic messages"),
    ];

    if json {
        let mut entries = Vec::new();
        for (name, needs_key, description) in providers {
            let has_key = if needs_key {
                keychain::get_key(name)?.is_some()
            } else {
                true
            };
            entries.push(json!({
                "name": name,
                "needs_key": needs_key,
                "key_set": has_key,
                "description": description,
            }));
        }
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        for (name, needs_key, description) in providers {
            let status = if !needs_key {
                "n/a       "
            } else if keychain::get_key(name)?.is_some() {
                "✓ key set "
            } else {
                "  no key  "
            };
            println!("{name:<12} {status}  {description}");
        }
    }
    Ok(())
}

fn providers_set_key(provider: &str, from_stdin: bool, json: bool) -> Result<()> {
    require_supported_real_provider(provider)?;

    let raw = if from_stdin {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("reading key from stdin")?;
        buf
    } else {
        rpassword::prompt_password(format!("Enter API key for {provider}: "))
            .context("reading key (interactive prompt)")?
    };
    let key = raw.trim();
    if key.is_empty() {
        bail!("empty key — nothing stored");
    }
    keychain::set_key(provider, key)?;

    if json {
        println!("{}", json!({ "provider": provider, "stored": true }));
    } else {
        println!("✓ stored key for {provider} in Keychain");
    }
    Ok(())
}

fn providers_get_key(provider: &str, reveal: bool, json: bool) -> Result<()> {
    require_supported_real_provider(provider)?;
    let key = keychain::get_key(provider)?.ok_or_else(|| anyhow!("no key set for `{provider}`"))?;

    if json {
        let value = if reveal {
            json!(key)
        } else {
            json!(mask_key(&key))
        };
        println!(
            "{}",
            json!({ "provider": provider, "key": value, "revealed": reveal })
        );
    } else if reveal {
        println!("{key}");
    } else {
        println!("{} (use --reveal to show full key)", mask_key(&key));
    }
    Ok(())
}

fn providers_remove_key(provider: &str, json: bool) -> Result<()> {
    require_supported_real_provider(provider)?;
    let removed = keychain::delete_key(provider)?;
    if json {
        println!("{}", json!({ "provider": provider, "removed": removed }));
    } else if removed {
        println!("✓ removed key for {provider}");
    } else {
        println!("(no key was set for {provider})");
    }
    Ok(())
}

fn providers_test(provider: &str, json: bool) -> Result<()> {
    require_supported_real_provider(provider)?;
    let key = require_key(provider)?;

    let prov: Box<dyn Provider> = match provider {
        "openai" => Box::new(OpenAiProvider::new(key)),
        "anthropic" => Box::new(AnthropicProvider::new(key)),
        _ => unreachable!("checked by require_supported_real_provider"),
    };

    let request = RewriteRequest {
        system: Some("Respond with exactly the word 'ok' and nothing else.".to_string()),
        user: "ping".to_string(),
        model: None,
    };
    let mut got_token = false;
    let mut got_done = false;
    let mut total_chars = 0usize;
    prov.rewrite(&request, &mut |event| match event {
        RewriteEvent::Token { text } => {
            got_token = true;
            total_chars += text.len();
        }
        RewriteEvent::Done => got_done = true,
        RewriteEvent::Usage { .. } => {}
    })?;

    if !(got_token && got_done) {
        bail!("{provider}: missing expected events (token={got_token} done={got_done})");
    }
    if json {
        println!(
            "{}",
            json!({ "provider": provider, "ok": true, "response_chars": total_chars })
        );
    } else {
        println!("✓ {provider} OK ({total_chars} response chars)");
    }
    Ok(())
}

fn require_supported_real_provider(name: &str) -> Result<()> {
    match name {
        "openai" | "anthropic" => Ok(()),
        other => bail!("providers commands operate on `openai` or `anthropic`; got `{other}`"),
    }
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return "***".to_string();
    }
    format!("{}…{}", &key[..3], &key[key.len() - 4..])
}
