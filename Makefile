.PHONY: server client www dev clean check

# ─── Говно-сервер ────────────────────────────────────────────────────────────
server:
	cargo run -p govno-server

server-release:
	cargo build -p govno-server --release

# ─── WASM-клиент ─────────────────────────────────────────────────────────────
client:
	wasm-pack build client --target web --out-dir ../www/pkg --dev

client-release:
	wasm-pack build client --target web --out-dir ../www/pkg --release

# ─── Статика ─────────────────────────────────────────────────────────────────
# После билда клиента открой http://localhost:8080
www: client
	cd www && python3 -m http.server 8080

# ─── Всё сразу (в двух терминалах) ──────────────────────────────────────────
# terminal 1:  make server
# terminal 2:  make www

# ─── Dev: только пересобрать клиент и серв ───────────────────────────────────
dev: client server

# ─── Проверки ────────────────────────────────────────────────────────────────
check:
	cargo check -p govno-server
	cargo check -p govno-client --target wasm32-unknown-unknown

clippy:
	cargo clippy -p govno-server
	cargo clippy -p govno-client --target wasm32-unknown-unknown

# ─── Cleanup ─────────────────────────────────────────────────────────────────
clean:
	cargo clean
	rm -rf www/pkg
