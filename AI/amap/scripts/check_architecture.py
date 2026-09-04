#!/usr/bin/env python3
"""Fail when functional-core crates acquire infrastructure dependencies or side effects."""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
PURE_CRATES = {
    "amap-domain",
    "amap-assurance",
    "amap-invariant",
    "amap-uncertainty",
}
FORBIDDEN_DEPENDENCIES = {
    "async-nats",
    "axum",
    "datafusion",
    "object_store",
    "rdkafka",
    "reqwest",
    "sqlx",
    "tokio",
    "tonic",
    "wasmtime",
}
FORBIDDEN_SOURCE_TOKENS = {
    "Utc::now(": "wall-clock access",
    "Uuid::new_v4(": "random identifier generation",
    "std::fs::": "filesystem access",
    "tokio::": "async runtime access",
    "reqwest::": "HTTP access",
    "sqlx::": "database access",
    "Command::new(": "process execution",
}


def metadata() -> dict:
    output = subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=ROOT,
        text=True,
    )
    return json.loads(output)


def main() -> int:
    failures: list[str] = []
    packages = {package["name"]: package for package in metadata()["packages"]}

    for name in sorted(PURE_CRATES):
        package = packages.get(name)
        if package is None:
            failures.append(f"missing core package: {name}")
            continue
        dependencies = {dependency["name"] for dependency in package["dependencies"]}
        forbidden = sorted(dependencies & FORBIDDEN_DEPENDENCIES)
        if forbidden:
            failures.append(f"{name} has infrastructure dependencies: {', '.join(forbidden)}")

        source_root = pathlib.Path(package["manifest_path"]).parent / "src"
        for source in source_root.rglob("*.rs"):
            text = source.read_text(encoding="utf-8")
            for token, reason in FORBIDDEN_SOURCE_TOKENS.items():
                if token in text:
                    relative = source.relative_to(ROOT)
                    failures.append(f"{name}: {relative} contains {reason} ({token})")

    for delivery in ("control-plane", "llm-gateway", "amap-workers"):
        package = packages.get(delivery)
        if package is None:
            failures.append(f"missing delivery package: {delivery}")
            continue
        dependencies = {dependency["name"] for dependency in package["dependencies"]}
        if "amap-cli" in dependencies:
            failures.append(f"{delivery} must depend on amap-platform, not amap-cli")

    if failures:
        for failure in failures:
            print(f"architecture violation: {failure}", file=sys.stderr)
        return 1

    print("architecture boundaries are valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
