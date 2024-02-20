prepare:
	rustup target add wasm32-unknown-unknown

build-contract:
	cargo +nightly build --release --target wasm32-unknown-unknown

check:
	cargo check

lint:
	cargo clippy
