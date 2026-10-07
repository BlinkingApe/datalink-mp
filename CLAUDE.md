## Layout

Full map in [CONTRIBUTING.md](CONTRIBUTING.md).

- Code: `crates/`, one folder per crate; `Cargo.toml` is the list.
- Docs: `docs/`, split by reader (`players/`, `contributors/`, `maintainers/`, `adr/`, `agents/`).
- Shipped README: `packaging/README.txt`, copied into each release archive.
- Archives: `docs/archive/` (evidence docs) and `.scratch/archive/` (finished efforts). The root `.ignore` hides both from Grep, Glob and `rg`, so searches skip them. For history, read those paths by name or search with `rg --no-ignore`.

## Agent skills

### Issue tracker

Issues live as local markdown files under `.scratch/<feature>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Default five-role vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`), recorded as a `Status:` line in each issue file. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `GLOSSARY.md` plus `docs/adr/` at the repo root. See `docs/agents/domain.md`.
