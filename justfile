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
    npm run registry:check

test-rust:
    cargo clippy -p happybirthdayvalera --all-targets -- -D warnings
    cargo clippy -p happybirthdayvalera --all-targets --features web --target wasm32-unknown-unknown -- -D warnings
    cargo clippy -p happybirthdayvalera --all-targets --features desktop -- -D warnings
    cargo test -p happybirthdayvalera
    cargo test -p happybirthdayvalera --features desktop

# ev_lib 0.34 enables nightly-only features; RUSTC_BOOTSTRAP lets the stable toolchain build it.
test-rust-ev:
    RUSTC_BOOTSTRAP=1 cargo clippy -p happybirthdayvalera --all-targets --features web,ev_uikit --target wasm32-unknown-unknown -- -D warnings
    RUSTC_BOOTSTRAP=1 cargo clippy -p happybirthdayvalera --all-targets --features desktop,ev_uikit -- -D warnings
    RUSTC_BOOTSTRAP=1 cargo test -p happybirthdayvalera --features desktop,ev_uikit

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

# Stands: a dictation composer per kit, on the public API of the crate (web only)
stand-dioxus-shadcn:
    cd examples/stand-dioxus-shadcn && dx serve --platform web --port 8201

# ev_lib 0.34 enables nightly-only features; RUSTC_BOOTSTRAP lets the stable toolchain build it.
# dx runs Tailwind before cargo, so cargo check first runs build.rs, which writes the kit CSS that Tailwind reads.
stand-dioxus-evinvest:
    RUSTC_BOOTSTRAP=1 cargo check -p stand-dioxus-evinvest --target wasm32-unknown-unknown
    cd examples/stand-dioxus-evinvest && RUSTC_BOOTSTRAP=1 dx serve --platform web --port 8202

# shadcn: clippy after dx, because asset! needs the assets/tailwind.css that the Tailwind step of dx writes.
# evinvest: clippy before dx, because its build.rs writes what Tailwind reads (see above).
stands-dioxus-build:
    cd examples/stand-dioxus-shadcn && dx build --platform web
    cargo clippy -p stand-dioxus-shadcn --all-targets --target wasm32-unknown-unknown -- -D warnings
    RUSTC_BOOTSTRAP=1 cargo clippy -p stand-dioxus-evinvest --all-targets --target wasm32-unknown-unknown -- -D warnings
    cd examples/stand-dioxus-evinvest && RUSTC_BOOTSTRAP=1 dx build --platform web

# Packaging checks; nothing is published
pack-check: build-ts
    cd packages/react && npm pack --dry-run
    cargo publish -p happybirthdayvalera --dry-run --features web --allow-dirty
    cargo publish -p happybirthdayvalera --dry-run --features desktop --allow-dirty
