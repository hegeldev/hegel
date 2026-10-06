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

check-tests:
    uv run pytest -q

check-coverage:
    uv run coverage run -m pytest -q tests --ignore=tests/hypothesis_port
    uv run coverage report

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

check: check-format check-lint check-tests check-coverage
