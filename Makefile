# Developer shortcuts. Run `make dev-server` and `make dev-web` in two terminals,
# then open http://localhost:5173 (Vite proxies /api to the Rust server on :3100).

.PHONY: dev-server dev-web test check types build docker up docs measure

dev-server:
	DATA_DIR=./data LOG_LEVEL=debug,sqlx=warn cargo run -p shoppingcart

dev-web:
	cd web && npm run dev

test:
	cargo test --workspace
	cd web && npm run check && npm test

check:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cd web && npm run check

# Regenerate TypeScript API types from the Rust structs (ts-rs).
types:
	cargo test -p shoppingcart --lib export_bindings

build:
	cd web && npm ci && npm run build
	cargo build --release -p shoppingcart

docker:
	docker compose build

up:
	docker compose up -d --build

docs:
	python3 -m venv .venv && .venv/bin/pip install -q -r docs/requirements.txt
	.venv/bin/mkdocs build --strict

measure:
	./scripts/measure.sh
