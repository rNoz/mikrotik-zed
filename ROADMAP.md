# ROADMAP — MikroTik RouterOS Script support for Zed

> Personal fork: `rNoz/mikrotik-zed`  
> Upstream: `keiras94/mikrotik-zed`  
> Grammar fork: `rNoz/mikrotik-rsc-grammar`  
> Grammar upstream: `keiras94/mikrotik-rsc-grammar`  

This file tracks the state of the fork and the next contributions I intend to make.  It cross-links upstream repos, AGENTS.md, and the `.overlay/` notes.

---

## Current status (verified 2026-08-21)

| Area | State | Evidence |
|------|-------|----------|
| Grammar fork published | ✅ Working | `rNoz/mikrotik-rsc-grammar` @ `80b79ad` |
| `property=value` parsed as `named_param` | ✅ Fixed | Grammar corpus + `make test-grammar` pass |
| Space-separated menu paths | ⚠️ Flat identifiers | Still parsed as `menu_argument` values; needs external scanner or verb list |
| Syntax highlighting queries | ✅ Fixed | `languages/rsc/highlights.scm` matches grammar; 59 corpus tests pass |
| Bracket / indent / outline queries | ✅ Fixed | Added/fixed in grammar fork; local copies compatible |
| WASM extension build | ✅ Builds | `cargo build --target wasm32-wasip1 --release` (needs `RUSTFLAGS=""` on macOS) |
| `rsc-ls` LSP binary | ✅ Builds & runs | `cargo build -p rsc-ls --release` |
| LSP initialize | ✅ Works | Returns capabilities |
| LSP completion | ✅ Works | Sub-menus, verbs, arguments, flags, enum/bool values |
| LSP hover | ✅ Works for properties | Full menu-path hover incomplete (root/intermediate paths not in `commands.toml`) |
| `commands.toml` extraction | ✅ Works | 685 menus, 8 root menus; Python tests pass |
| Rust tests / clippy | ✅ Pass | `cargo test --workspace` (8 LSP tests), `cargo clippy --workspace --all-targets -- -D warnings` |
| Python extraction tests | ✅ Pass | `pytest tests/ -p no:libtmux` |
| Zed "Install Dev Extension" | ⬜ Not tested here | Requires running Zed GUI manually |
| Publish to zed-industries/extensions | ⬜ Not started | Needs final `extension.toml` + PR |

### Fixed in this pass

1. `extension.toml` pointed to a local bare repo (`/Users/Francisco/...`) — replaced with public `rNoz/mikrotik-rsc-grammar` and the new commit.
2. Local `highlights.scm` / `outline.scm` referenced grammar nodes (`root_menu`, `sub_menu`, `menu_continuation`) that do not exist in the published grammar — rewritten to use `menu_path`, `menu_command`, `menu_argument`, `global_command_name`, etc.
3. LSP completion JSON used snake-case `insert_text` / `insert_text_format` instead of LSP-standard camelCase — fixed with `#[serde(rename_all = "camelCase")]`.
4. LSP hover was completely broken because it built context with wrong line/character arguments — fixed.
5. WASM extension embedded and parsed the entire `commands.toml` even though it never used it — `src/lib.rs` is now a thin launcher; WASM binary is smaller.
6. `Makefile` failed on macOS when `~/.cargo/config.toml` sets `-mmacosx-version-min` — added `RUSTFLAGS=""` to the WASM targets.
7. `Makefile` still referenced the removed `grammars/rsc/` directory — rewritten to clone the external grammar for `test-grammar` / `validate`.
8. `AGENTS.md` had a stale `fravic/mikrotik-rsc-grammar` link — fixed to `keiras94` / `rNoz` fork.
9. Added 7 LSP unit tests (`lsp/src/main.rs`) covering tokenization, multi-line context building, and menu-path parsing.
10. Fixed LSP tokenizer so `key="value with spaces"` is handled as one token; fixed command parsing so action commands like `/ip route check` and global-command arguments like `:put $x` are classified correctly.
11. **Forked and patched the grammar:** `property=value` in menu commands now parses as `(named_param ...)`, `=` and `->` no longer collide with the generic `operator` token, and `subexpression` correctly interleaves operators/identifiers with values. 59 tree-sitter corpus tests pass.
12. Added missing grammar query files (`brackets.scm`, `indents.scm`, `outline.scm`) to the fork.
13. Fixed LSP root-menu completion: typing `/` now returns the 8 root menus instead of verbs (lone `/` maps to empty root path in `parse_line`).
14. Updated `.agents/skills/zed-extension-dev.md` repo URLs and test command.

### Known limitations

* Hover over a partial menu path (e.g. `/ip` or the `address` segment in `/ip address ...`) returns nothing, because only leaf menus are present in `commands.toml`.  Intermediate/root entries would need to be synthesized or added during extraction.
* Space-separated menu paths are not structurally parsed; verbs and path segments are indistinguishable in the parse tree.
* `rsc-ls` must be on PATH; there is no download/install helper for end users.
* The language is called `MikroTik Script` in `config.toml`; AGENTS.md still says it should be `RSC`.  Decide and align.

---

## Planned contributions (5 PR strategy)

The goal is to upstream small, logical, easy-to-review PRs rather than one large patch. The first two grammar PRs are independent and can be opened in parallel. The first two extension PRs are also independent. The fifth PR depends on having a release process.

### 1. Grammar: metadata and query completeness

- **Repo:** `keiras94/mikrotik-rsc-grammar`
- **Scope:** No `grammar.js` behavior changes. Add missing `name` to `tree-sitter.json`, fix repository link, add missing query files, fix `highlights.scm` to use existing node names.
- **Why first:** Tiny, safe, and makes later grammar changes easier to review because the query surface is stable.
- **Files:** `tree-sitter.json`, `queries/*.scm`, generated parser.
- **Drafts:** `.overlay/issues/grammar-metadata-and-queries.md`, `.overlay/prs/grammar-metadata-and-queries.md`.

### 2. Grammar: parse `property=value` in menu commands

- **Repo:** `keiras94/mikrotik-rsc-grammar`
- **Scope:** Modify `grammar.js` so `key=value` inside menu commands binds as `(named_param ...)`. Split `=` and `->` into dedicated tokens. Update corpus expectations and queries.
- **Why second:** This is the headline Phase 1 quality fix. It is self-contained and directly testable via corpus.
- **Files:** `grammar.js`, `src/parser.c`, `test/corpus/`, `queries/highlights.scm`.
- **Drafts:** `.overlay/issues/named-param-menu-commands.md`, `.overlay/prs/named-param-menu-commands.md`.

### 3. Extension: LSP correctness fixes

- **Repo:** `keiras94/mikrotik-zed`
- **Scope:** Fix camelCase JSON in `CompletionItem` and broken hover context. Independent of grammar changes.
- **Why third:** Small, high-value bug fixes that make the LSP actually work with Zed. Easy to review.
- **Files:** `lsp/src/completion.rs`, `lsp/src/hover.rs`.
- **Drafts:** `.overlay/issues/lsp-correctness.md`, `.overlay/prs/lsp-correctness.md`.

### 4. Extension: LSP tokenizer and command parser fixes + tests

- **Repo:** `keiras94/mikrotik-zed`
- **Scope:** Fix quoted-value tokenization, action-command classification, and add regression tests in `lsp/src/main.rs`. Independent of grammar changes.
- **Why fourth:** Adds test coverage and fixes real parsing edge cases. Still self-contained.
- **Files:** `lsp/src/main.rs`.
- **Drafts:** `.overlay/issues/lsp-tokenizer-tests.md`, `.overlay/prs/lsp-tokenizer-tests.md`.

### 5. Extension: align with public grammar and add `rsc-ls` distribution

- **Repo:** `keiras94/mikrotik-zed`
- **Scope:** Two halves that can be split if desired:
  - **5a.** Point `extension.toml` and `Makefile` to the public grammar, align query files, update docs. Depends on grammar PRs 1 and 2.
  - **5b.** Add GitHub Actions release builds for `rsc-ls` and implement download/cache in `src/lib.rs`. This unblocks publishing to `zed-industries/extensions`.
- **Why fifth:** This is the integration PR. It ties together the grammar fixes and makes the extension usable by end users.
- **Files:** `extension.toml`, `Makefile`, `languages/rsc/*.scm`, `src/lib.rs`, `.github/workflows/release.yml`, `AGENTS.md`, `CONTRIBUTING.md`.
- **Drafts:** `.overlay/issues/align-queries-with-grammar.md`, `.overlay/prs/align-queries-with-grammar.md`, `.overlay/issues/rsc-ls-distribution.md`, `.overlay/prs/rsc-ls-distribution.md`.

## Order and dependency graph

```
Grammar PR 1: metadata + queries          Extension PR 3: LSP correctness
       |                                            |
       v                                            v
Grammar PR 2: named_param parsing            Extension PR 4: tokenizer + tests
       |                                            |
       +--------------------+-----------------------+
                            v
              Extension PR 5: public grammar + rsc-ls distribution
```

Grammar PR 1 and PR 2 should target `keiras94/mikrotik-rsc-grammar`.
Extension PR 3 and PR 4 can target `keiras94/mikrotik-zed` immediately.
Extension PR 5 waits for the grammar PRs to land upstream (or keeps pointing to the `rNoz` fork if upstream is slow).

## Cross-project linking

- Grammar issue/PR drafts reference the extension PR draft at `rNoz/mikrotik-zed/.overlay/prs/align-queries-with-grammar.md`.
- Extension issue/PR drafts reference grammar drafts at `rNoz/mikrotik-rsc-grammar/.overlay/prs/grammar-metadata-and-queries.md` and `rNoz/mikrotik-rsc-grammar/.overlay/prs/named-param-menu-commands.md`.
- Both projects now have `CONTRIBUTING.md` and `.overlay/PLAYBOOK.md` that share the same issue/PR style rules (no hard-wrapped paragraphs, no emojis, no em dashes).

## Deferred work (post-publication)

- Space-separated menu paths (likely needs external scanner or verb list).
- Hover over intermediate/root menu paths (needs synthesizing parent entries in `commands.toml`).
- Diagnostics / linting for unknown paths or properties.
- Language naming alignment (`RSC` vs `MikroTik Script`).

---

## Cross-references

* `AGENTS.md` — project operating guide (updated).
* `CONTRIBUTING.md` — contribution rules, including the no-hard-wrap rule for issue/PR bodies.
* `.agents/skills/zed-extension-dev.md` — Zed-specific dev notes (updated).
* `.overlay/PLAYBOOK.md` — how we prepare branches and PRs for this repo.
* `.overlay/overlay.conf` — git-overlay config; `check_cmds` now pass.
* `.overlay/issues/` and `.overlay/prs/` — draft issue and PR descriptions for the 5 planned contributions.
* `data/commands.toml` — generated command table (truth source: `llms-full.txt`).
* `lsp/src/` — pure Rust language server.
* `languages/rsc/` — Zed query files.
* Sibling project: `~/projects/mikrotik-hub/mikrotik-rsc-grammar/.overlay/` holds grammar-side plans.

## Active upstream contributions

| PR | Repo | Status | What it does |
|---|---|---|---|
| `keiras94/mikrotik-rsc-grammar#2` | grammar | draft | Metadata/query completeness; cherry-picked into `overlay/integration` as `aad25c0`. |
| `keiras94/mikrotik-zed#3` | extension | draft | camelCase completion JSON + hover context; cherry-picked into `overlay/integration` as `abd2478`. |
| `keiras94/mikrotik-zed#4` | extension | draft | Tokenizer quoted values + action commands + tests; cherry-picked into `overlay/integration` as `7cefeb2`. |

## How to test right now
See `.overlay/TESTING.md` for step-by-step Zed GUI validation. The integrated branch is `overlay/integration`.

## Next decision
After the Zed GUI test passes, mark the three draft PRs ready for review and move to extension PR 5 (public grammar + `rsc-ls` distribution).

## Decision log

* **Grammar source:** Fork `keiras94/mikrotik-rsc-grammar` to `rNoz/mikrotik-rsc-grammar` so we can patch `grammar.js` without waiting for upstream.  The extension points to the fork until the changes are upstreamed.
* **Grammar fix strategy:** Split `=` and `->` into dedicated tokens (`assignment`, `arrow`) and add a `menu_argument` rule so `named_param` wins over `_value`.  This fixes the dominant `property=value` case without an external scanner.
* **WASM extension scope:** Keep it as a thin launcher; do not duplicate command data in WASM. The native `rsc-ls` binary owns the data.
* **macOS build workaround:** `RUSTFLAGS=""` in `Makefile` instead of editing global `~/.cargo/config.toml`, because the global flag is needed for other projects.
* **Space-separated paths:** Deferred.  A structural fix likely requires an external scanner or a curated verb list; accept query-time limitation for now.
