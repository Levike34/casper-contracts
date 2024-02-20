prepare:
	rustup target add wasm32-unknown-unknown

build-contract:
	cargo build --release --target wasm32-unknown-unknown

test:
	cargo build --release --target wasm32-unknown-unknown
	cp target/wasm32-unknown-unknown/release/stake_contract.wasm contract_tests/tests/wasm
	cd contract_tests/tests && cargo test

check:
	cargo check

lint:
	cargo clippy