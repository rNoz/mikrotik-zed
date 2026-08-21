# Integration Branch — MikroTik Zed Extension

## What is this file?
This document records the current integrated state of all topic branches so you can test the extension in the real Zed GUI without relying on chat history.

## Current integration branch
`overlay/integration` in `~/projects/mikrotik-hub/mikrotik-zed`.

## What is cherry-picked into it?

| Topic branch | Commit on integration | What it brings |
|---|---|---|
| `rnoz/lsp-correctness` | `abd2478` | LSP `CompletionItem` camelCase JSON; hover context uses real line/character. |
| `rnoz/lsp-tokenizer-tests` | `7cefeb2` | Tokenizer handles `key="value with spaces"`; action commands (`/ip route check`) classified correctly; 8 LSP unit tests. |

Base of integration is `keiras94/mikrotik-zed@099ffb5`.

## How to test in real Zed right now

### 1. Build the language server binary
```bash
cd ~/projects/mikrotik-hub/mikrotik-zed
git checkout overlay/integration
cargo build -p rsc-ls --release
```
The binary is at `target/release/rsc-ls`.

### 2. Make `rsc-ls` available on PATH
Zed launches `rsc-ls` via `worktree.which("rsc-ls")`. The easiest way is to symlink it somewhere on your PATH:
```bash
mkdir -p ~/.local/bin
ln -sf ~/projects/mikrotik-hub/mikrotik-zed/target/release/rsc-ls ~/.local/bin/rsc-ls
# Ensure ~/.local/bin is on PATH in the shell that starts Zed.
```

### 3. Build the WASM extension
```bash
cd ~/projects/mikrotik-hub/mikrotik-zed
RUSTFLAGS="" cargo build --target wasm32-wasip1 --release
cp target/wasm32-wasip1/release/mikrotik_zed.wasm extension.wasm
```

### 4. Install as a dev extension in Zed
1. Open Zed.
2. Run the action `extensions: install dev extension` (Cmd-Shift-P → type it).
3. Select `~/projects/mikrotik-hub/mikrotik-zed`.
4. Open any `.rsc` file.

### 5. What to verify manually
- **Syntax highlighting** works for comments, menu paths (`/ip address add`), properties (`address=10.0.0.1/24`), strings, variables.
- **Bracket matching** for `()`, `[]`, `{}`.
- **Outline view** shows menu paths and global commands.
- **Completion**: type `/ip address add ` and see property/flag suggestions; type `/` alone and see root menus.
- **Hover**: hover over `address` in `/ip address add address=10.0.0.1/24` and see a markdown tooltip.
- **Quoted values**: type `comment="hello world"` and confirm it is treated as one token/argument.
- **Action commands**: type `/ip route check` and confirm no further path suggestions appear after `check`.

### 6. Check Zed logs if something fails
- `zed: open log` in the command palette.
- Or start Zed from the terminal with `zed --foreground` and watch stderr.

## Known gaps (expected, not bugs to report yet)
- Hover over partial menu paths (`/ip`, `/ip address`) returns nothing because `commands.toml` only contains leaf menus.
- Space-separated menu paths (`/ip address add`) are structurally flat in the grammar; outline/heuristics are limited.
- The extension does not download `rsc-ls` automatically; you must keep it on PATH.

## Next steps after manual Zed test
1. If the integration works, mark the upstream PRs ready for review:
   - `keiras94/mikrotik-zed#3`
   - `keiras94/mikrotik-zed#4`
2. Open the grammar PR upstream as non-draft if not already:
   - `keiras94/mikrotik-rsc-grammar#2`
3. Once those land, work on PR 5 (public grammar + `rsc-ls` distribution) so end users do not need to build from source.

## Rebuild integration after topic branch changes
```bash
cd ~/projects/mikrotik-hub/mikrotik-zed
git overlay sync
# Recompose if needed; see .overlay/PLAYBOOK.md
```
