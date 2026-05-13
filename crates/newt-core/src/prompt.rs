//! Prompts: parse, list, and seed defaults.
//!
//! Prompts are plain Markdown files with YAML frontmatter, one per file:
//!
//! ```text
//! ---
//! name: Improve writing
//! emoji: ✨
//! description: ...
//! model: anthropic/claude-sonnet-4-6   # optional
//! ---
//! Body of pure instructions: what to do with the user's selected text.
//! Do not embed the selection here — it is sent automatically as a
//! separate user message. See `provider::RewriteRequest`.
//! ```
//!
//! The eight default prompts are embedded in the binary at compile time and
//! seeded into the user's prompts directory on first run if it's empty.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::paths::Paths;

/// The eight bundled default prompts: (filename, file contents).
///
/// We list them explicitly rather than walking the directory so we don't pull
/// in `include_dir` and so the embedded set is auditable at a glance.
pub const DEFAULT_PROMPTS: &[(&str, &str)] = &[
    (
        "improve-writing.md",
        include_str!("defaults/prompts/improve-writing.md"),
    ),
    (
        "fix-grammar.md",
        include_str!("defaults/prompts/fix-grammar.md"),
    ),
    (
        "make-concise.md",
        include_str!("defaults/prompts/make-concise.md"),
    ),
    (
        "make-formal.md",
        include_str!("defaults/prompts/make-formal.md"),
    ),
    (
        "make-casual.md",
        include_str!("defaults/prompts/make-casual.md"),
    ),
    (
        "summarize.md",
        include_str!("defaults/prompts/summarize.md"),
    ),
    (
        "bullet-points.md",
        include_str!("defaults/prompts/bullet-points.md"),
    ),
    (
        "translate-en.md",
        include_str!("defaults/prompts/translate-en.md"),
    ),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromptFrontmatter {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub emoji: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Prompt {
    /// Filename minus the `.md` extension. Stable identifier used by the CLI.
    pub id: String,
    pub name: String,
    pub description: String,
    pub emoji: Option<String>,
    pub model: Option<String>,
    /// The prompt's instruction body, sent verbatim as the LLM's system
    /// message. The user's selection is sent as a separate user message and
    /// is **not** interpolated into this string — see `RewriteRequest`.
    pub instructions: String,
}

/// Split a frontmatter document into (yaml, body).
///
/// Format: starts with `---\n`, frontmatter, then `\n---\n` (or `\n---` at EOF),
/// then the body. Trailing whitespace on the closing fence is tolerated.
fn split_frontmatter(input: &str) -> Result<(&str, &str)> {
    let s = input
        .strip_prefix("---\n")
        .or_else(|| input.strip_prefix("---\r\n"))
        .ok_or_else(|| anyhow!("missing opening `---` frontmatter fence"))?;

    // Find the closing fence: a line that is exactly `---` (allowing CR).
    let mut start = 0usize;
    for line in s.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed == "---" {
            let yaml = &s[..start];
            let body_start = start + line.len();
            let body = if body_start <= s.len() {
                &s[body_start..]
            } else {
                ""
            };
            return Ok((yaml, body));
        }
        start += line.len();
    }
    Err(anyhow!("missing closing `---` frontmatter fence"))
}

/// Parse a single prompt file's contents. `id` is the filename without `.md`.
pub fn parse(id: &str, contents: &str) -> Result<Prompt> {
    let (yaml, body) =
        split_frontmatter(contents).with_context(|| format!("parsing prompt `{id}`"))?;

    let fm: PromptFrontmatter = serde_yml::from_str(yaml)
        .with_context(|| format!("parsing frontmatter of prompt `{id}`"))?;

    Ok(Prompt {
        id: id.to_string(),
        name: fm.name,
        description: fm.description,
        emoji: fm.emoji,
        model: fm.model,
        instructions: body.trim_matches('\n').to_string(),
    })
}

/// Read every `*.md` file in the prompts directory, parse it, and return the
/// list sorted by display name (case-insensitive). Missing directory is not an
/// error — it just yields an empty list, since `list()` is read-only.
pub fn list(paths: &Paths) -> Result<Vec<Prompt>> {
    let dir = paths.prompts_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("invalid prompt filename: {}", path.display()))?
            .to_string();
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        out.push(parse(&id, &contents)?);
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    Ok(out)
}

/// Load a single prompt by id (filename stem). Looks on disk first; falls
/// back to the bundled default with the matching id if no on-disk file
/// exists. Returns an error only if `id` is unknown to both sources.
pub fn load(paths: &Paths, id: &str) -> Result<Prompt> {
    let on_disk = paths.prompts_dir().join(format!("{id}.md"));
    if on_disk.exists() {
        let contents = std::fs::read_to_string(&on_disk)
            .with_context(|| format!("reading {}", on_disk.display()))?;
        return parse(id, &contents);
    }
    for (filename, contents) in DEFAULT_PROMPTS {
        if filename.trim_end_matches(".md") == id {
            return parse(id, contents);
        }
    }
    Err(anyhow!(
        "no prompt with id `{id}` (not on disk, not a default)"
    ))
}

/// Write the bundled default prompts into the prompts directory if and only if
/// it's empty. Idempotent: re-running has no effect on a populated directory.
pub fn seed_defaults_if_empty(paths: &Paths) -> Result<usize> {
    paths.ensure_dirs()?;
    let dir = paths.prompts_dir();
    let is_empty = std::fs::read_dir(&dir)?.next().is_none();
    if !is_empty {
        return Ok(0);
    }
    write_defaults(&dir)?;
    Ok(DEFAULT_PROMPTS.len())
}

fn write_defaults(dir: &Path) -> Result<()> {
    for (filename, contents) in DEFAULT_PROMPTS {
        std::fs::write(dir.join(filename), contents)
            .with_context(|| format!("writing default prompt {filename}"))?;
    }
    Ok(())
}
