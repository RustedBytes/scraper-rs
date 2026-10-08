"""Sequential AB/BA runs of two checkouts with identical benchmark sources."""

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import time
from pathlib import Path


def command(args, cwd):
    return subprocess.check_output(args, cwd=cwd, text=True).strip()


def harness_hash(root):
    paths = sorted((root / "benchmarks").glob("*.rs"))
    return {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}


def schedule(repeats):
    return [
        (("base", "candidate") if i % 2 == 0 else ("candidate", "base"))
        for i in range(repeats)
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("base", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--repeats", type=int, default=6)
    parser.add_argument("--filter", default="")
    parser.add_argument("--settle-seconds", type=float, default=30.0)
    parser.add_argument("--controlled-host", action="store_true")
    args = parser.parse_args()
    if args.settle_seconds < 0:
        parser.error("settle-seconds must be nonnegative")
    if args.repeats < 4 or args.repeats % 2:
        parser.error("use an even repeat count >= 4 for balanced AB/BA")
    roots = {"base": args.base.resolve(), "candidate": args.candidate.resolve()}
    if roots["base"] == roots["candidate"]:
        parser.error("base and candidate must be separate checkouts")
    if not harness_hash(roots["base"]):
        parser.error("benchmark sources are missing")
    if harness_hash(roots["base"]) != harness_hash(roots["candidate"]):
        parser.error(
            "benchmark sources differ; apply the identical harness to both checkouts"
        )
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    metadata = {
        "schema": 1,
        "controlled_host_asserted": args.controlled_host,
        "platform": platform.platform(),
        "cpu": platform.processor(),
        "cpuinfo": Path("/proc/cpuinfo").read_text()
        if Path("/proc/cpuinfo").exists()
        else None,
        "rustc": command(["rustc", "-Vv"], roots["base"]),
        "environment": {
            k: os.environ.get(k)
            for k in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "PYO3_PYTHON")
        },
        "schedule": schedule(args.repeats),
        "filter": args.filter,
        "settle_seconds": args.settle_seconds,
        "revisions": {},
    }
    for label, root in roots.items():
        metadata["revisions"][label] = {
            "sha": command(["git", "rev-parse", "HEAD"], root),
            "rustc": command(["rustc", "-Vv"], root),
            "diff": command(["git", "diff", "HEAD"], root),
            "status": command(["git", "status", "--short"], root),
            "harness": harness_hash(root),
            "build_config": {
                str(p.relative_to(root)): p.read_text()
                for p in (
                    root / "build.rs",
                    root / "rust-toolchain.toml",
                    root / ".cargo/config.toml",
                )
                if p.exists()
            },
        }
        shutil.copytree(
            root / "benchmarks",
            args.output / f"{label}-harness",
            ignore=shutil.ignore_patterns("__pycache__", "*.json"),
        )
        shutil.copyfile(root / "Cargo.lock", args.output / f"{label}-Cargo.lock")
        shutil.copyfile(root / "Cargo.toml", args.output / f"{label}-Cargo.toml")
    (args.output / "metadata.json").write_text(json.dumps(metadata, indent=2))
    if (
        metadata["revisions"]["base"]["rustc"]
        != metadata["revisions"]["candidate"]["rustc"]
    ):
        parser.error("compiler versions differ; use the same toolchain")
    # Prebuild both versions before starting the balanced timing schedule.
    for label, root in roots.items():
        env = dict(os.environ, CARGO_TARGET_DIR=str(args.output / f"build-{label}"))
        with (args.output / f"build-{label}.log").open("w") as log:
            subprocess.run(
                ["cargo", "bench", "--locked", "--bench", "boundaries", "--no-run"],
                cwd=root,
                env=env,
                stdout=log,
                stderr=subprocess.STDOUT,
                check=True,
            )
    time.sleep(args.settle_seconds)
    for repeat, order in enumerate(metadata["schedule"]):
        for label in order:
            run = args.output / f"{repeat:02d}-{label}"
            run.mkdir()
            env = dict(os.environ, CARGO_TARGET_DIR=str(args.output / f"build-{label}"))
            cmd = [
                "cargo",
                "bench",
                "--locked",
                "--bench",
                "boundaries",
                "--",
                args.filter,
                "--save-baseline",
                f"repeat-{repeat}-{label}",
            ]
            (run / "command.json").write_text(json.dumps(cmd))
            with (run / "stdout.log").open("w") as log:
                subprocess.run(
                    cmd,
                    cwd=roots[label],
                    env=env,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                    check=True,
                )
            shutil.copytree(
                Path(env["CARGO_TARGET_DIR"]) / "criterion", run / "criterion"
            )
    print(args.output)


if __name__ == "__main__":
    main()
