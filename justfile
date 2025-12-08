init:
    uv venv --python python3.14
    uv pip install --upgrade pip setuptools wheel

install: init
    uv pip install maturin

build:
    uvx maturin build --release --compatibility linux

build_manylinux:
    docker run --rm \
        -v "$PWD":/io \
        -w /io \
        ghcr.io/pyo3/maturin:latest \
        build --release --strip --compatibility manylinux2014

install-wheel: build
    uv pip uninstall scraper-rs
    uv pip install target/wheels/scraper_rs-0.1.2-cp310-abi3-manylinux_2_17_x86_64.manylinux2014_x86_64.whl

test:
    uv run pytest tests/test_scraper.py
