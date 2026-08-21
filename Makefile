.PHONY: help generate test test-grammar test-rust test-python test-all parse highlight extract build check build-lsp clean install-dev validate

# ── Tree-sitter grammar ────────────────────────────────────────

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

generate: ## Regenerate parser.c from grammar.js
	cd grammars/rsc && npx tree-sitter generate

test: test-grammar ## Alias for test-grammar

test-grammar: ## Run tree-sitter grammar tests (corpus tests)
	cd grammars/rsc && npx tree-sitter test

test-rust: ## Run Rust extension tests
	cargo test --lib

test-python: ## Run Python extraction tests
	python3 -m pytest tests/ -v

test-all: test-grammar test-rust test-python ## Run all tests

parse: ## Parse a file (usage: make parse FILE=grammars/rsc/test/example.rsc)
	cd grammars/rsc && npx tree-sitter parse ../..$(FILE)

highlight: ## Highlight a file (usage: make highlight FILE=grammars/rsc/test/example.rsc)
	cd grammars/rsc && npx tree-sitter highlight ../..$(FILE)

# ── Command extraction ─────────────────────────────────────────

extract: ## Regenerate data/commands.toml from llms-full.txt
	python3 scripts/extract_commands.py

# ── Build ──────────────────────────────────────────────────────

build: ## Build WASM extension
	cargo build --target wasm32-wasip1 --release
	cp target/wasm32-wasip1/release/mikrotik_zed.wasm extension.wasm

build-lsp: ## Build native LSP binary
	cargo build -p rsc-ls --release
	@echo "Binary: target/release/rsc-ls"

check: ## Quick compile verification
	cargo check --target wasm32-wasip1
	cargo check -p rsc-ls

# Note: a global `[build] rustflags` in ~/.cargo/config.toml that sets a
# macOS deployment target (e.g. -mmacosx-version-min=15.3) will be passed
# to the wasm32-wasip1 linker and cause the build to fail. Keep macOS flags
# under [target.*-apple-darwin] only, or use a temporary CARGO_HOME that does
# not contain such rustflags.

# ── Cleanup ────────────────────────────────────────────────────

clean: ## Remove all build artifacts and generated files
	rm -rf target/
	rm -f extension.wasm
	rm -f Cargo.lock
	cd grammars/rsc && rm -rf target/ build/ src/grammar.json src/node-types.json src/parser.c

# ── Development ────────────────────────────────────────────────

install-dev: ## Point Zed to this directory (manual: Zed > Install Dev Extension)
	@echo "Open Zed → Command Palette → 'Install Dev Extension' → select this directory"
	@echo ""
	@echo "Make sure rsc-ls binary is in PATH:"
	@echo "  cargo build -p rsc-ls --release"
	@echo "  export PATH=\"\$$PWD/target/release:\$$PATH\""

validate: generate test-all extract ## Full validation: regenerate + all tests + extract
	@echo "All checks passed."
