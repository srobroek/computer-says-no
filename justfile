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
bench:
    cargo run -- benchmark

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
serve:
    cargo run -- serve

[group('run')]
classify *ARGS:
    cargo run -- classify {{ARGS}}

# Clean

[group('clean')]
clean:
    cargo clean
