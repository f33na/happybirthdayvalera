set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

# Install JS dependencies for the workspace
install:
    npm ci

# Regenerate the shared CSS and the Whisper worker for both libraries
sync-assets:
    node scripts/sync-assets.mjs

# Fail when a generated file differs from what its source produces
check-drift: sync-assets
    git diff --exit-code -- packages/react/src/styles.css crates/happybirthdayvalera/assets crates/happybirthdayvalera/shared shared/wave-fixture.json
    test -z "$(git status --porcelain -- packages/react/src/styles.css crates/happybirthdayvalera/assets crates/happybirthdayvalera/shared shared/wave-fixture.json)"

# All unit tests
test: test-ts test-rust

test-ts:
    npm run -w happybirthdayvalera typecheck
    npm run -w happybirthdayvalera lint
    npm run -w happybirthdayvalera test

test-rust:
    cargo clippy -p happybirthdayvalera --all-targets --features web -- -D warnings
    cargo clippy -p happybirthdayvalera --all-targets --features desktop -- -D warnings
    cargo test -p happybirthdayvalera --features desktop

# Recognition tests over fixtures/: downloads the Whisper models on the first run
test-accuracy:
    npm run -w happybirthdayvalera test:accuracy
    cargo test -p happybirthdayvalera --features desktop -- --ignored

build-ts:
    npm run -w happybirthdayvalera build

demo-react:
    npm run -w react-demo dev

demo-web:
    cd examples/dioxus-demo && dx serve --platform web

demo-desktop:
    cd examples/dioxus-demo && dx serve --platform desktop

# Packaging checks; nothing is published
pack-check: build-ts
    cd packages/react && npm pack --dry-run
    cargo publish -p happybirthdayvalera --dry-run --features desktop --allow-dirty
