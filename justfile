# `just` prints bash comments in stdout by default. this suppresses that
set ignore-comments := true

# Per-component recipes live next to the code they drive and run from that
# directory: `just libhegel::test`, `just rust::test`, `just python::test`,
# `just typescript::test`.
# Recipes here are the ones that span the whole Cargo workspace (libhegel,
# the Rust frontend and the Python extension share one workspace and
# lockfile; the coverage report covers the first two).

# The libhegel engine
mod libhegel
# The Rust frontend
mod rust 'frontends/rust'
# The Python frontend (needs uv; `just python::setup` first)
mod python 'frontends/python'
# The TypeScript frontend (needs Node; `just typescript::setup` first)
mod typescript 'frontends/typescript'

# Definition of done for the whole repo: lint + every component's tests.
check: check-lint libhegel::check rust::check python::check typescript::check

# Lints spanning the Cargo workspace, plus each component's own.
check-lint: check-format check-clippy check-nocov-style check-test-modules check-internal-asserts check-release-script libhegel::check-lint rust::check-lint python::check-lint typescript::check-lint

format:
    cargo fmt --all
    just python::format
    just typescript::format
    # also run format-nix if we have nix installed
    @which nix && just format-nix || true

format-nix:
    nix run nixpkgs#nixfmt -- nix/flake.nix

check-format:
    cargo fmt --all --check

check-format-nix:
    nix run nixpkgs#nixfmt -- --check nix/flake.nix

check-clippy:
    cargo clippy --workspace --all-features --all-targets -- -D warnings
    # Without --all-features so the default mode's engine loader (compiled
    # out under `static-engine`, which --all-features enables) is linted too.
    cargo clippy --workspace --all-targets -- -D warnings

check-nocov-style:
    scripts/check-nocov-style.py

check-test-modules:
    scripts/check-test-modules.py

check-internal-asserts:
    scripts/check-internal-asserts.py

check-release-script:
    .github/scripts/test_release.py

# Coverage is measured over the whole workspace: the engine is covered both
# by its own tests and by the Rust frontend driving it over the C ABI.

# Whole-workspace line coverage (needs cargo-llvm-cov + llvm-tools-preview).
check-coverage:
    # requires cargo-llvm-cov and llvm-tools-preview
    # Force opt-level 0 (overriding `[profile.dev] opt-level` in Cargo.toml):
    # cargo-llvm-cov needs unoptimized code for accurate line attribution, as
    # optimization inlines small functions and drops their coverage regions.
    CARGO_PROFILE_DEV_OPT_LEVEL=0 scripts/check-coverage.py

# Test with the minimal versions of every dependency in the workspace.
check-tests-minimal-versions:
    # Generate the minimal-versions lockfile (requires nightly), then test against
    # exactly that lockfile (--locked stops cargo regenerating it for non-minimal
    # versions). Note this rewrites the checked-in Cargo.lock; restore it with
    # `git checkout Cargo.lock` afterwards.
    cargo generate-lockfile -Z minimal-versions
    # The workspace run includes libhegel's dlopen smoke tests, which need
    # the cdylib `cargo test` alone does not produce (see `c-test-smoke` in
    # libhegel/justfile).
    cargo build --locked -p hegeltest-c
    # The Python extension's tests are its pytest suite; with --all-features
    # its `extension-module` build would not even link outside maturin.
    HEGEL_RUNNING_TESTS_WITH_RUST_NIGHTLY=1 RUST_BACKTRACE=1 cargo test --workspace --exclude hegel-python --locked --all-features

# Run cargo-insta, installing it first if it's missing. Pinned to the same
# version as the `insta` dev-dependency so snapshot format stays consistent.
INSTA_VERSION := "1.47.2"
insta *ARGS:
    @cargo install --quiet --locked --version {{INSTA_VERSION}} cargo-insta
    cargo insta {{ARGS}}

# These aliases are provided as ux improvements for local developers. CI
# should use the longer forms.

test: libhegel::test rust::test python::check-tests typescript::check-test
coverage: check-coverage
lint: check-lint
