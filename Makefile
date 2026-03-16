.PHONY: orchestrator workers-all wasm www dev clean check fmt

orchestrator:
	GOVNO_TOKEN=говно cargo run -p govno-orchestrator

liquid:
	cargo run -p govno-worker-liquid

solid:
	cargo run -p govno-worker-solid

gas:
	cargo run -p govno-worker-gas

critical:
	cargo run -p govno-worker-critical

wasm:
	wasm-pack build client --target web --out-dir ../www/pkg --dev

wasm-release:
	wasm-pack build client --target web --out-dir ../www/pkg --release

www: wasm
	cd www && python3 -m http.server 8080

check:
	cargo check -p govno-orchestrator
	cargo check -p govno-client --target wasm32-unknown-unknown
	cargo check -p workers_common

clippy:
	cargo clippy -p govno-orchestrator -- -D warnings
	cargo clippy -p govno-client --target wasm32-unknown-unknown -- -D warnings

fmt:
	cargo fmt --all

clean:
	cargo clean
	rm -rf www/pkg
