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

# A stand is a standalone consumer app of the built package, so the package is built first
stand-react-shadcn: build-ts
    npm run -w stand-react-shadcn dev

stand-react-evinvest: build-ts
    npm run -w stand-react-evinvest dev

# Build the package, then typecheck and build both React stands
stands-react-build: build-ts
    npm run -w stand-react-shadcn typecheck
    npm run -w stand-react-shadcn build
    npm run -w stand-react-evinvest typecheck
    npm run -w stand-react-evinvest build

demo-web:
    cd examples/dioxus-demo && dx serve --platform web

demo-desktop:
    cd examples/dioxus-demo && dx serve --platform desktop

# Packaging checks; nothing is published
pack-check: build-ts
    cd packages/react && npm pack --dry-run
    cargo publish -p happybirthdayvalera --dry-run --features desktop --allow-dirty
