"""Harness validation without requiring timing claims."""

import importlib.util
from pathlib import Path


def load(relative):
    spec = importlib.util.spec_from_file_location(
        "benchmark_tool", Path(__file__).parents[1] / relative
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_balanced_schedule():
    tool = load("tools/repeat_benchmarks.py")
    assert tool.schedule(4) == [("base", "candidate"), ("candidate", "base")] * 2


def test_raw_samples_preserved():
    tool = load("benchmarks/python_wrappers.py")
    assert tool.summarize([10, 30, 20]) == {
        "samples_ns_per_op": [10, 30, 20],
        "median_ns_per_op": 20,
    }


def test_harness_hash_detects_changes(tmp_path):
    tool = load("tools/repeat_benchmarks.py")
    (tmp_path / "benchmarks").mkdir()
    fixture = tmp_path / "benchmarks/common.rs"
    fixture.write_text("first")
    before = tool.harness_hash(tmp_path)
    fixture.write_text("second")
    assert before != tool.harness_hash(tmp_path)


def test_runner_rejects_unbalanced_repeats(tmp_path):
    import subprocess
    import sys

    script = Path(__file__).parents[1] / "tools/repeat_benchmarks.py"
    result = subprocess.run(
        [
            sys.executable,
            str(script),
            str(tmp_path),
            str(tmp_path),
            str(tmp_path / "output"),
            "--repeats",
            "3",
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 2
    assert "balanced AB/BA" in result.stderr
    assert not (tmp_path / "output").exists()


def test_runner_retains_all_runs_and_lockfiles(tmp_path, monkeypatch):
    import json
    import sys

    tool = load("tools/repeat_benchmarks.py")
    roots = [tmp_path / "base", tmp_path / "candidate"]
    for root in roots:
        (root / "benchmarks").mkdir(parents=True)
        (root / "benchmarks/common.rs").write_text("same harness")
        (root / "Cargo.lock").write_text(root.name)
        (root / "Cargo.toml").write_text("manifest")
    output = tmp_path / "results"
    monkeypatch.setattr(
        sys, "argv", ["runner", *(str(r) for r in roots), str(output), "--repeats", "4"]
    )
    monkeypatch.setattr(tool, "command", lambda args, cwd: "recorded")
    monkeypatch.setattr(tool.time, "sleep", lambda seconds: None)
    calls = []

    def fake_run(cmd, **kwargs):
        if "--no-run" in cmd:
            return
        calls.append(kwargs["cwd"].name)
        criterion = Path(kwargs["env"]["CARGO_TARGET_DIR"]) / "criterion"
        criterion.mkdir(parents=True, exist_ok=True)
        (criterion / "sample.json").write_text('{"times": [100, 200]}')

    monkeypatch.setattr(tool.platform, "processor", lambda: "test CPU")
    monkeypatch.setattr(tool.platform, "platform", lambda: "test OS")
    monkeypatch.setattr(tool.subprocess, "run", fake_run)
    tool.main()
    assert calls == ["base", "candidate", "candidate", "base"] * 2
    assert len(list(output.glob("[0-9][0-9]-*/criterion/sample.json"))) == 8
    assert (output / "base-Cargo.lock").read_text() == "base"
    assert (output / "candidate-Cargo.lock").read_text() == "candidate"
    assert (
        json.loads((output / "metadata.json").read_text())["controlled_host_asserted"]
        is False
    )


def test_python_wrapper_harness_uses_real_extension():
    import asyncio

    tool = load("benchmarks/python_wrappers.py")
    result = asyncio.run(tool.run(iterations=2, repeats=2))
    assert result["matches"] == 100
    assert len(result["extension_sha256"]) == 64
    assert set(result["metrics"]) == {
        "asyncio_ready_control",
        "core_select_existing",
        "public_select_existing",
        "core_parse_and_close",
        "public_parse_and_close",
        "python_wrap_elements_only",
    }
    for metric in result["metrics"].values():
        assert len(metric["samples_ns_per_op"]) == 2
        assert all(sample >= 0 for sample in metric["samples_ns_per_op"])
