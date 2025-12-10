init:
    uv venv --clear --python python3.14

install: init
    uv sync --group dev
    uv pip install --upgrade pip setuptools wheel

build: clean
    rm -rf target/wheels/scraper_rust-*.whl
    uv run maturin build --release --compatibility linux

build_manylinux:
    docker run --rm \
        -v "$PWD":/io \
        -w /io \
        ghcr.io/pyo3/maturin:latest \
        build --release --strip --compatibility manylinux2014

publish:
    docker run --rm \
        -v "$PWD":/io \
        -w /io \
        -e MATURIN_PYPI_TOKEN \
        ghcr.io/pyo3/maturin:latest \
        publish --skip-existing --compatibility manylinux2014

install-wheel: build
    uv pip uninstall scraper-rust
    uv pip install target/wheels/scraper_rust-*.whl

test:
    uv run pytest tests/

fmt:
    cargo fmt --all
    uv run ruff format

lint:
    cargo clippy --all-targets --all-features -- -D warnings

clean:
    rm -rf target
    rm -rf dist
    rm -rf .uv-cache .uv_cache
    rm -rf .ruff_cache
    rm -rf .pytest_cache
    rm -rf **/__pycache__
