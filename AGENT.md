# AGENT.md - LLM Development Guide for scraper-rs

This file contains essential information for Large Language Models (LLMs) to effectively work with the scraper-rs project.

## Project Overview

**scraper-rs** is a Python library providing high-performance HTML parsing and CSS/XPath selection through Python bindings to the Rust `scraper` crate. It uses PyO3 to create Python bindings for Rust code, offering zero Python-side parsing overhead.

- **Repository**: https://github.com/RustedBytes/scraper-rs
- **Package Name**: `scraper-rust` (PyPI)
- **Module Name**: `scraper_rs` (Python import)
- **License**: MIT
- **Python Support**: 3.10, 3.11, 3.12, 3.13, 3.14
- **ABI**: abi3 (CPython 3.10+)

## Technology Stack

### Core Technologies
- **Rust** (2024 edition)
  - `scraper` v0.25 - HTML parsing and CSS selection
  - `sxd-document` v0.3.2, `sxd-xpath` v0.4.2, `sxd_html` v0.1.2 - XPath support
  - `pyo3` v0.27 with `extension-module` and `abi3-py310` features
- **Python** 3.10+
  - Type annotations and stub files (`scraper_rs.pyi`, `py.typed`)
  - Async support via `asyncio` module
- **Build System**
  - `maturin` - Build PyO3 wheels
  - `uv` - Package/project management (preferred)
  - `just` - Command runner for common tasks

### Development Tools
- **Testing**: `pytest` with `pytest-asyncio`
- **Formatting**: `cargo fmt`, `ruff format`
- **Linting**: `cargo clippy`

## Project Structure

```
scraper-rs/
├── src/
│   └── lib.rs              # Main Rust source (PyO3 bindings)
├── tests/
│   ├── test_scraper.py     # Synchronous API tests
│   └── test_asyncio.py     # Asynchronous API tests
├── examples/
│   ├── demo.py             # Sync usage examples
│   └── demo_asyncio.py     # Async usage examples
├── scraper_rs/             # Python package directory (built by maturin)
├── scraper_rs.pyi          # Type stub file
├── py.typed                # PEP 561 marker for type support
├── Cargo.toml              # Rust package configuration
├── pyproject.toml          # Python package configuration
├── justfile                # Task automation (build, test, etc.)
├── uv.lock                 # UV dependency lock file
└── README.md               # User documentation
```

### Key Files

- **`src/lib.rs`**: Single Rust source file containing all PyO3 bindings
  - `Document` class: Main HTML document wrapper
  - `Element` class: Snapshot of HTML element with tag, text, HTML, and attributes
  - Helper functions: `parse()`, `select()`, `select_first()`, `first()`, `xpath()`, `xpath_first()`
  - Size limit enforcement (default 1 GiB, configurable via `max_size_bytes`)
  - Truncation support (`truncate_on_limit` parameter)

- **`scraper_rs.pyi`**: Type stubs for IDE support and type checking

- **`Cargo.toml`**: Rust crate configuration
  - `crate-type = ["cdylib"]` for Python extension module
  - Release optimizations: LTO, `codegen-units = 1`, `opt-level = "z"`

- **`pyproject.toml`**: Python package metadata and build configuration
  - Maturin build backend
  - Package name: `scraper-rust`
  - Keywords: html, scraping, css selectors, xpath, pyo3, rust

## Build and Development

### Setup

```bash
# Initialize virtual environment with uv (recommended)
just init

# Or manually with venv
python -m venv .venv
source .venv/bin/activate

# Install dependencies
just install
# Or: uv sync --group dev
```

### Building

```bash
# Build a release wheel (Linux compatibility)
just build

# Build manylinux wheel using Docker
just build_manylinux

# Install the built wheel
just install-wheel
```

**Alternative manual build:**
```bash
maturin build --release --compatibility linux
pip install target/wheels/scraper_rust-*.whl
```

### Development Workflow

```bash
# Build and install in development mode (editable)
source .venv/bin/activate
maturin develop --release --locked

# Run all tests
just test
# Or: uv run pytest tests/

# Format code
just fmt
# Or: cargo fmt --all && uv run ruff format

# Lint Rust code
just lint
# Or: cargo clippy --all-targets --all-features -- -D warnings

# Clean build artifacts
just clean
```

## Testing

### Test Structure
- **`tests/test_scraper.py`**: Core functionality tests
  - Document properties
  - CSS selection (`.select()`, `.select_first()`, `.find()`)
  - XPath selection (`.xpath()`, `.xpath_first()`)
  - Element methods (`.attr()`, `.get()`, `.to_dict()`)
  - Top-level helper functions
  - Size limits and truncation
  - Context manager support

- **`tests/test_asyncio.py`**: Async wrapper tests
  - `asyncio.parse()`, `asyncio.select()`, `asyncio.xpath()`

### Running Tests

```bash
# Run all tests
pytest tests/

# Run specific test file
pytest tests/test_scraper.py

# Run specific test
pytest tests/test_scraper.py::test_document_properties

# Run with verbose output
pytest tests/ -v
```

### CI/CD
- **GitHub Actions**: `.github/workflows/tests.yml`
  - Runs on Ubuntu latest
  - Tests against Python 3.10, 3.11, 3.12, 3.13, 3.14
  - Steps: checkout → setup Python/Rust → install deps → build with maturin → run pytest

## API Design

### Core Classes

**`Document`**
- Constructor: `Document(html: str, max_size_bytes: int = 1GiB, truncate_on_limit: bool = False)`
- Properties: `.html`, `.text`
- Methods:
  - `.select(css: str) -> list[Element]` - CSS selection
  - `.select_first(css: str) -> Element | None` - First CSS match
  - `.find(css: str)` - Alias for `.select_first()`
  - `.css(css: str)` - Alias for `.select()`
  - `.xpath(expr: str) -> list[Element]` - XPath selection
  - `.xpath_first(expr: str) -> Element | None` - First XPath match
  - `.close()` - Explicitly free resources (also supports context manager)

**`Element`**
- Properties: `.tag`, `.text`, `.html`, `.attrs`
- Methods:
  - `.attr(name: str) -> str | None` - Get single attribute
  - `.get(name: str, default: str | None) -> str | None` - Get with default
  - `.to_dict() -> dict` - Serialize to dict
  - `.select(css)`, `.select_first(css)`, `.find(css)`, `.css(css)` - Nested CSS
  - `.xpath(expr)`, `.xpath_first(expr)` - Nested XPath

### Top-Level Functions
- `parse(html, max_size_bytes, truncate_on_limit) -> Document`
- `select(html, css, max_size_bytes, truncate_on_limit) -> list[Element]`
- `select_first(html, css, max_size_bytes, truncate_on_limit) -> Element | None`
- `first(html, css, max_size_bytes, truncate_on_limit) -> Element | None`
- `xpath(html, expr, max_size_bytes, truncate_on_limit) -> list[Element]`
- `xpath_first(html, expr, max_size_bytes, truncate_on_limit) -> Element | None`

### Async API
Module: `scraper_rs.asyncio`
- All top-level functions available as async versions
- `parse()` yields to event loop between operations
- `select()` and `xpath()` run in thread pool
- Same keyword arguments as sync versions

## Code Conventions

### Rust Code (`src/lib.rs`)
- Use `#[pyclass]` for Python-exposed classes
- Use `#[pymethods]` for Python-exposed methods
- Use `#[getter]` for property accessors
- Document public APIs with doc comments (`///`)
- Follow Rust naming conventions (snake_case for functions/variables)
- Use PyO3 error types (`PyValueError`, `PyResult`)
- Release build optimizations in `Cargo.toml` for smaller binary size

### Python Code
- Type annotations required (uses `scraper_rs.pyi` stub file)
- Follow PEP 8 style guidelines
- Use descriptive variable names
- Include docstrings for public functions
- Use pytest fixtures for test data

### Memory Management
- `Element` stores owned data (no lifetime issues)
- `Document.close()` explicitly frees resources
- Context manager protocol supported: `with Document(html) as doc:`
- Default size limit: 1 GiB (`DEFAULT_MAX_PARSE_BYTES`)
- Truncation respects UTF-8 character boundaries

## Common Development Tasks

### Adding a New Method to Document
1. Add Rust implementation in `src/lib.rs` under `impl Document`
2. Add `#[pymethods]` annotation if needed
3. Update `scraper_rs.pyi` with type signature
4. Add tests in `tests/test_scraper.py`
5. Update README.md with usage example
6. Run `maturin develop` to rebuild
7. Run `pytest tests/` to verify

### Adding a New Top-Level Function
1. Add Rust function in `src/lib.rs` with `#[pyfunction]`
2. Register with `m.add_function(wrap_pyfunction!(function_name, m)?)?` in `scraper_rs` module
3. Export in `__all__` if applicable
4. Update `scraper_rs.pyi`
5. Add tests
6. Update documentation

### Debugging Build Issues
- Ensure Rust toolchain is installed: `rustc --version`
- Check maturin version: `maturin --version` (requires >=1.5, <2.0)
- Clear build cache: `just clean` or `rm -rf target/`
- Check PyO3 compatibility with Python version
- Verify `Cargo.lock` is up to date

### Release Process
See `.github/workflows/release.yml` and `.github/workflows/bump-version.yml`
- Version is specified in both `Cargo.toml` and `pyproject.toml` (must match)
- Manylinux wheels built via Docker with official maturin image
- Published to PyPI via `maturin publish`

## Important Notes for LLMs

1. **This is a dual-language project**: Rust for core functionality, Python for the API
2. **PyO3 limitations**: Not all Rust types can cross the FFI boundary - use owned data
3. **Size limits**: Always respect `max_size_bytes` to prevent memory issues
4. **UTF-8 safety**: Truncation must respect character boundaries
5. **No async in Rust**: Async is Python-only; Rust code is synchronous
6. **Single source file**: All Rust code is in `src/lib.rs` (keep it organized)
7. **Type stubs are critical**: Always update `scraper_rs.pyi` when changing APIs
8. **Tests are comprehensive**: Follow existing test patterns in `tests/`
9. **Build artifacts**: `target/` and `.venv/` should not be committed
10. **Use `just` commands**: They handle common workflows correctly

## Dependencies

### Rust Dependencies (Cargo.toml)
- `pyo3 = { version = "0.27", features = ["extension-module", "abi3-py310"] }`
- `scraper = "0.25"`
- `sxd-document = "0.3.2"`
- `sxd-xpath = "0.4.2"`
- `sxd_html = "0.1.2"`

### Python Dependencies (pyproject.toml)
**Runtime**: None (self-contained binary wheel)
**Development**:
- `maturin` - Build tool
- `pytest` - Testing framework
- `pytest-asyncio>=1.3.0` - Async test support

## Troubleshooting

### Build Failures
- **Missing Rust**: Install via `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Maturin version mismatch**: `pip install --upgrade maturin`
- **PyO3 errors**: Check Python version compatibility (3.10+ required)

### Test Failures
- **Import errors**: Rebuild with `maturin develop --release`
- **Missing pytest**: `pip install pytest pytest-asyncio`
- **Async tests fail**: Ensure `pytest-asyncio>=1.3.0` is installed

### Runtime Issues
- **Module not found**: Install wheel with `pip install target/wheels/scraper_rust-*.whl`
- **Size limit errors**: Increase `max_size_bytes` or use `truncate_on_limit=True`
- **Memory usage**: HTML parsing creates owned copies; use `.close()` when done

## Resources

- **PyO3 Documentation**: https://pyo3.rs/
- **scraper crate**: https://docs.rs/scraper/
- **Maturin Guide**: https://www.maturin.rs/
- **Project README**: `/home/runner/work/scraper-rs/scraper-rs/README.md`
- **Example Usage**: `/home/runner/work/scraper-rs/scraper-rs/examples/demo.py`
