init:
    uv venv --python python3.14
    uv pip install --upgrade pip setuptools wheel

install: init
    uv pip install maturin

build:
    rm -rf target/wheels/scraper_rust-*.whl
    uv run maturin build --release --compatibility linux

build_manylinux:
    docker run --rm \
        -v "$PWD":/io \
        -w /io \
        ghcr.io/pyo3/maturin:latest \
        build --release --strip --compatibility manylinux2014

install-wheel: clean build
    uv pip uninstall scraper-rust
    uv pip install target/wheels/scraper_rust-*.whl

test:
    uv run pytest tests/test_scraper.py

clean:
    rm -rf target
    rm -rf dist
    rm -rf .uv-cache .uv_cache
    rm -rf .ruff_cache
    rm -rf .pytest_cache
    rm -rf **/__pycache__
