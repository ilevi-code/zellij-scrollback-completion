WASM := target/wasm32-wasip1/release/zellij-scrollback-completion.wasm

.PHONY: build test check clean

# Release wasm build (requires: rustup target add wasm32-wasip1)
build: $(WASM)

$(WASM): Cargo.toml $(wildcard src/*.rs)
	cargo build --release --target wasm32-wasip1

# Host-side checks (no wasm target needed)
test:
	cargo test

check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings

clean:
	cargo clean
