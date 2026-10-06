set ignore-comments := true

# Create the virtualenv and build the extension in development mode.
setup:
    uv sync --group dev
    uv run maturin develop --uv

# Rebuild the Rust extension after changing src/.
build:
    uv run maturin develop --uv

build-release:
    uv run maturin develop --uv --release

test *ARGS:
    uv run pytest {{ARGS}}

# The package's own test suite.
check-tests:
    uv run pytest -q tests --ignore=tests/hypothesis_port

# The package's own test suite, requiring complete line coverage.
check-coverage:
    uv run coverage run -m pytest -q tests --ignore=tests/hypothesis_port
    uv run coverage report

# The Hypothesis test suite, ported to run on hegel.hypothesis.
check-hypothesis-port:
    uv run pytest -q tests/hypothesis_port -p no:cacheprovider

format:
    uv run ruff format python tests
    uv run ruff check --fix python tests
    cargo fmt

check-format:
    uv run ruff format --check python tests
    cargo fmt --check

check-lint:
    uv run ruff check python tests
    cargo clippy --all-targets -- -D warnings

check-types:
    uv run mypy python/hegel

# Unit tests for the release automation in .github/scripts.
check-release-script:
    uv run python .github/scripts/test_release.py

check: check-format check-lint check-types check-release-script check-coverage check-hypothesis-port
