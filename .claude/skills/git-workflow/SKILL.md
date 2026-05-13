---
name: git-workflow
description: How to ship a change in this repo — branch, commit, issue, PR. Trigger when the user asks to "create a PR", "ship this", "open a PR", "commit and push", or any multi-step git workflow that ends with a PR.
---

# Newt git workflow

Solo-dev, build-in-public indie project. Keep everything brief and casual.

## Conventions

- **Base branch is `develop`**, not `main`. PRs target `develop`. Never push to `develop` directly.
- **Branch names**: `<type>/<kebab-case-summary>` — e.g. `feat/phase-0-scaffold`, `fix/prompt-loader-panic`, `chore/bump-deps`. No ticket prefixes.
- **Conventional commits** for both commit messages AND PR titles. The PR title becomes the squash-merge commit, so make it useful in `git log`. GitHub appends the `(#N)` PR number on squash-merge — don't add it manually.
- **Tone**: brief, casual, build-in-public. One-line PR sections are ideal. No corporate boilerplate. **No `Co-Authored-By: Claude` trailer** unless the user explicitly asks.

## Standard order

1. **Branch off `develop`**: `git checkout -b feat/<summary>`
2. **Stage explicit paths only** — never `git add -A` or `git add .`. List the paths you want.
3. **Commit** with a conventional-commits message. Body usually unnecessary.
4. **Push** with `-u`: `git push -u origin <branch>`. (User has authorized push-on-PR-request.)
5. **Create issue** with `gh issue create --repo bogdan-pistol/newt-app`. 2–4 lines max. Reference PRD section if relevant. No labels, no template. Capture the issue number.
6. **Create PR** with `gh pr create --base develop --head <branch>`. Use the What/Why/Notes template (see `.github/pull_request_template.md`). Reference the issue with `Closes #N` in the Why section. Skip Notes if there's nothing to add.

## Never stage these

- `product/scracth.json` — user's personal scratch file. Always untracked, leave alone.
- Secrets, `.env`, large binaries.

Note: `.claude/` IS tracked in this repo (skills are committed). Stage skill files normally.

## Safety

- Never `--no-verify` or skip hooks.
- Never amend or rebase existing commits — add a new commit instead.
- Never force-push.
- If a step fails, stop and report — don't paper over with destructive commands.

## PR body shape

See `.github/pull_request_template.md`. Three `##` sections:
- **What** — one line, the noun version of what changed.
- **Why** — one line motivation; include `Closes #N`.
- **Notes** — only if there's something non-obvious. Skip otherwise.
