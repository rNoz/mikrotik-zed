# What is next — MikroTik Zed Extension

> Last updated: 2026-08-21.

## Immediate next action for the operator
Test the integrated branch in the real Zed GUI. Follow `.overlay/TESTING.md`.

## If the GUI test passes
1. Mark these draft PRs as ready for review:
   - `keiras94/mikrotik-zed#3` — fix(lsp): emit camelCase completion JSON and fix hover context
   - `keiras94/mikrotik-zed#4` — fix(lsp): handle quoted values and action commands correctly, add tests
   - `keiras94/mikrotik-rsc-grammar#2` — chore: add missing tree-sitter metadata and query files
2. Monitor upstream for review feedback and respond quickly.

## If the GUI test fails
1. Reproduce the failure and capture logs (see `.overlay/TESTING.md` § Troubleshooting).
2. Fix the bug on the relevant topic branch, force-push the topic branch, and re-cherry-pick onto `overlay/integration`.
3. Do not mark the PRs ready for review until the failure is understood.

## After the first three PRs land upstream
1. **Extension PR 5** — public grammar alignment + `rsc-ls` distribution. This is the big integration PR that makes the extension installable by end users.
   - Point `extension.toml` and `Makefile` to the upstream grammar commit.
   - Add a GitHub Actions workflow to build `rsc-ls` for macOS aarch64/x86_64 and Linux aarch64/x86_64.
   - Implement download/cache of `rsc-ls` in `src/lib.rs` so users do not need to build it.
   - Update `AGENTS.md` and `CONTRIBUTING.md` with the new process.
2. **Optional grammar follow-up** — add a GitHub Action to run `tree-sitter generate && tree-sitter test` on PRs.

## Longer-term backlog
- Space-separated menu paths: decide whether to add an external scanner or a verb list.
- Hover over intermediate/root menu paths: synthesize parent entries in `commands.toml` or during extraction.
- Diagnostics / linting for unknown paths or properties.
- Language naming alignment (`RSC` vs `MikroTik Script`).
- Publish to `zed-industries/extensions`.

## Where the truth lives
- Code state: `overlay/integration` branches in both repos.
- Upstream PRs/issues: `keiras94/mikrotik-zed` and `keiras94/mikrotik-rsc-grammar`.
- Local strategy: `ROADMAP.md` and `.overlay/PLAYBOOK.md`.
