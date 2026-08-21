# Testing the MikroTik Zed Extension in the Real Zed GUI

> Last updated: 2026-08-21. See also `.overlay/INTEGRATION.md` for the integrated branch state.

## Quick start

```bash
cd ~/projects/mikrotik-hub/mikrotik-zed
git checkout overlay/integration

# 1. Build the native language server
cargo build -p rsc-ls --release

# 2. Put it on PATH
mkdir -p ~/.local/bin
ln -sf ~/projects/mikrotik-hub/mikrotik-zed/target/release/rsc-ls ~/.local/bin/rsc-ls

# 3. Build the WASM extension
RUSTFLAGS="" cargo build --target wasm32-wasip1 --release
cp target/wasm32-wasip1/release/mikrotik_zed.wasm extension.wasm
```

Then in Zed:
1. Cmd-Shift-P → `extensions: install dev extension`
2. Pick `~/projects/mikrotik-hub/mikrotik-zed`
3. Open any `.rsc` file.

## What to test

### Phase 1 — Grammar / highlighting
Create or open a file with this content:
```rsc
# Simple demo
/ip address add address=10.0.0.1/24 interface=ether1 comment="office LAN"
/ip route add dst-address=0.0.0.0/0 gateway=192.168.1.1
:put $myVar
```

Verify:
- Comments are colored as comments.
- `/ip address add` is highlighted (menu path + command).
- `address=...`, `interface=...`, `dst-address=...` are highlighted as properties/values.
- `"office LAN"` is one string, not broken at the space.
- `$myVar` is highlighted as a variable.

### Phase 2 — LSP completion
Type these sequences and trigger completion (Ctrl-Space or just type):
1. `/` alone → should show 8 root menus (`interface`, `ip`, `ipv6`, `queue`, `routing`, `system`, `tool`, `user`).
2. `/ip address add ` → should show properties (`address`, `network`, `netmask`, `broadcast`, `interface`) and flags.
3. `/ip route check` → after `check`, no more path suggestions (it is an action command).
4. `add comment="hello world"` → the quoted value stays attached to `comment`.

### Phase 3 — LSP hover
Hover over:
- `address` in `/ip address add address=10.0.0.1/24` → should show a markdown tooltip with type info.
- `/ip/address` at the start of a line → should show the menu entry if the whole path is a known leaf menu.

### Phase 4 — Outline / symbols
Open the outline panel and confirm:
- `/ip address add ...` appears as a menu-path item.
- `:put ...` appears as a global-command item.

### Phase 5 — Brackets and indentation
Type multi-line blocks and confirm bracket matching and indentation:
```rsc
:if ($a > 1) do={
  :put "big"
}
```

## What is expected to NOT work yet
- Hover over partial paths (`/ip`, `/ip address`) returns nothing.
- Space-separated menu paths are structurally flat; outline will not show nested sub-menus for them.
- `rsc-ls` is not downloaded automatically; you must keep the binary on PATH.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| No completions/hover | `rsc-ls` not on PATH | Symlink it to `~/.local/bin` and restart Zed |
| Zed says extension failed to load | WASM not built or stale | Re-run the `RUSTFLAGS="" cargo build ...` + `cp` step |
| Highlighting looks wrong | Query files out of sync with grammar | Run `make test-grammar` in the extension repo |
| Changes not picked up | Dev extension cached | `extensions: reload extensions` or restart Zed |

## Capturing evidence
If something fails:
1. Run `zed --foreground` from the terminal and reproduce.
2. Copy the stderr/output to a file in `.overlay/notes/`.
3. Run `zed: open log` and copy relevant lines.
4. Attach the file to the relevant upstream issue or PR.
