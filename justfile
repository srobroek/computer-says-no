set dotenv-load

# Default recipe
default:
    @just --list

# Build

[group('build')]
build:
    cargo build

[group('build')]
release:
    cargo build --release

# Test

[group('test')]
test:
    cargo test

[group('test')]
bench *ARGS:
    cargo run -- benchmark run {{ARGS}}

[group('test')]
bench-generate:
    cargo run -- benchmark generate-datasets --sets-dir ./reference-sets

# Lint

[group('lint')]
clippy:
    cargo clippy -- -D warnings

[group('lint')]
fmt:
    cargo fmt --check

[group('lint')]
fmt-fix:
    cargo fmt

[group('lint')]
audit:
    cargo audit

[group('lint')]
lint: clippy fmt

# Check (full suite)

[group('check')]
check: lint test build

# Run

[group('run')]
mcp:
    cargo run -- mcp

[group('run')]
classify *ARGS:
    cargo run -- classify {{ARGS}}

# Clean

[group('clean')]
clean:
    cargo clean
