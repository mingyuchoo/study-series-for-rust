"""Shell integration tests with an isolated, recording Cargo executable.

These fixtures validate orchestration only; real Rust verification is separate.
Run: python tests/run-scripts/test_run_scripts.py --source <app-root>
"""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


SOURCE = Path(__file__).resolve().parents[2]
BASH = shutil.which("bash")
if os.name == "nt":
    BASH = "C:/Program Files/Git/bin/bash.exe"
PWSH = shutil.which("pwsh")


def ps_literal(value):
    return "'" + str(value).replace("'", "''") + "'"


class RunScripts(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="run scripts fixture ")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.root = self.base / "project with spaces"
        self.root.mkdir()
        shutil.copytree(SOURCE / "scripts", self.root / "scripts")
        shutil.copytree(SOURCE / ".agents", self.root / ".agents")
        shutil.copy2(SOURCE / "Cargo.toml", self.root / "Cargo.toml")
        self.bin = self.base / "mock bin"
        self.bin.mkdir()
        self.log = self.base / "calls.jsonl"
        recorder = self.bin / "record.py"
        recorder.write_text(
            "import json, os, sys\n"
            "with open(os.environ['CARGO_CALL_LOG'], 'a', encoding='utf-8') as f:\n"
            "    f.write(json.dumps({'cwd': os.getcwd(), 'args': sys.argv[1:]}) + '\\n')\n"
            "args = sys.argv[1:]\n"
            "stage = args[0] if args else ''\n"
            "if stage == 'test': stage = 'doc' if '--doc' in args else 'test'\n"
            "sys.exit(int(os.environ.get('FAIL_CODE', '37')) if stage == os.environ.get('FAIL_STAGE') else 0)\n",
            encoding="utf-8",
        )
        # Git Bash receives POSIX argv; PowerShell receives native Windows argv.
        shell_quote = lambda s: "'" + str(s).replace("'", "'\\''") + "'"
        (self.bin / "cargo").write_text(
            "#!/usr/bin/env bash\nexec "
            + shell_quote(Path(sys.executable).as_posix())
            + " " + shell_quote(recorder.as_posix()) + ' "$@"\n',
            encoding="utf-8", newline="\n",
        )
        (self.bin / "cargo").chmod(0o755)
        (self.bin / "cargo.cmd").write_text(
            '@echo off\r\n"' + sys.executable + '" "' + str(recorder) + '" %*\r\n',
            encoding="utf-8",
        )
        self.cwd = self.base / "unrelated directory"
        self.cwd.mkdir()

    def invoke(self, shell, arguments=(), failure=None, script="run"):
        if self.log.exists():
            self.log.unlink()
        env = os.environ.copy()
        env["PATH"] = str(self.bin) + os.pathsep + env["PATH"]
        env["CARGO_CALL_LOG"] = str(self.log)
        env.pop("FAIL_STAGE", None)
        if failure:
            env["FAIL_STAGE"] = failure
            env["FAIL_CODE"] = "37"
        if shell == "bash":
            self.assertTrue(BASH and Path(BASH).exists(), "Bash required")
            command = [BASH, str(self.root / ("scripts/" + script + ".sh")), *arguments]
        else:
            self.assertIsNotNone(PWSH, "PowerShell 7.4+ required")
            driver = self.base / "driver.ps1"
            driver.write_text(
                "& " + ps_literal(self.root / ("scripts/" + script + ".ps1")) + " "
                + " ".join(ps_literal(arg) for arg in arguments)
                + "\nexit $LASTEXITCODE\n", encoding="utf-8",
            )
            command = [PWSH, "-NoProfile", "-File", str(driver)]
        result = subprocess.run(command, cwd=self.cwd, env=env, capture_output=True, text=True)
        calls = [json.loads(line) for line in self.log.read_text(encoding="utf-8").splitlines()] if self.log.exists() else []
        return result, calls

    @staticmethod
    def stages(calls):
        return ["doc" if "--doc" in call["args"] else call["args"][0] for call in calls]

    def test_default_order_and_locked_resolution(self):
        """AC-01: complete pipeline, production feature isolation, locked resolution."""
        for shell in ("bash", "pwsh"):
            with self.subTest(shell=shell):
                result, calls = self.invoke(shell)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(self.stages(calls), ["fmt", "clippy", "test", "doc", "build", "run"])
                for call in calls[1:]:
                    self.assertIn("--locked", call["args"])
                self.assertEqual(calls[0]["args"], ["fmt", "--all"])
                self.assertIn("--all-targets", calls[2]["args"])
                self.assertIn("test-support", calls[2]["args"])
                self.assertIn("test-support", calls[3]["args"])
                self.assertEqual(calls[4]["args"], ["build", "--locked"])
                self.assertEqual(calls[5]["args"], ["run", "--locked"])

    def test_foreign_cwd_spaces_and_app_arguments(self):
        """AC-02: caller location does not matter and app arguments stay distinct."""
        args = ["--help", "two words", "--no-run", "apostrophe's value", "한글 값"]
        for shell in ("bash", "pwsh"):
            with self.subTest(shell=shell):
                result, calls = self.invoke(shell, ["--", *args])
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(calls[-1]["args"], ["run", "--locked", "--", *args])
                for call in calls:
                    self.assertEqual(Path(call["cwd"]).resolve(), self.root.resolve())

    def test_every_stage_fails_fast_and_preserves_exit(self):
        """AC-03: each failing stage returns its code without executing later work."""
        stages = ["fmt", "clippy", "test", "doc", "build", "run"]
        for shell in ("bash", "pwsh"):
            for index, failure in enumerate(stages):
                with self.subTest(shell=shell, failure=failure):
                    result, calls = self.invoke(shell, failure=failure)
                    self.assertEqual(result.returncode, 37, result.stdout + result.stderr)
                    self.assertEqual(self.stages(calls), stages[:index + 1])

    def test_no_run_keeps_every_validation_stage(self):
        """AC-04: no-run removes only GUI startup."""
        for shell in ("bash", "pwsh"):
            with self.subTest(shell=shell):
                result, calls = self.invoke(shell, ["--no-run"])
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(self.stages(calls), ["fmt", "clippy", "test", "doc", "build"])

    def test_help_does_not_execute_commands(self):
        """AC-04: help documents public options and does no Cargo work."""
        for shell in ("bash", "pwsh"):
            with self.subTest(shell=shell):
                result, calls = self.invoke(shell, ["--help"])
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(calls, [])
                self.assertIn("--no-run", result.stdout)
                self.assertIn("--help", result.stdout)

    def test_invalid_option_does_not_execute_commands(self):
        """AC-04: invalid options fail before mutations."""
        for shell in ("bash", "pwsh"):
            for option in ("--invalid", "unexpected"):
                with self.subTest(shell=shell, option=option):
                    result, calls = self.invoke(shell, [option])
                    self.assertNotEqual(result.returncode, 0)
                    self.assertEqual(calls, [])

    def init_parent_repository(self, tracked_manifest=True):
        (self.base / ".gitignore").write_text("**/.artifacts/\n/mock bin/\n/driver.ps1\n/calls.jsonl\n", encoding="utf-8")
        for args in (
            ["init", "--quiet"],
            ["config", "user.email", "fixture@example.invalid"],
            ["config", "user.name", "Fixture"],
            ["add", "."],
        ):
            subprocess.run(["git", *args], cwd=self.base, check=True, capture_output=True)
        if not tracked_manifest:
            subprocess.run(["git", "rm", "--cached", "project with spaces/Cargo.toml"], cwd=self.base, check=True, capture_output=True)
            # An ignored Cargo manifest is present yet untracked and tree is clean.
            with (self.base / ".gitignore").open("a", encoding="utf-8") as f:
                f.write("/project with spaces/Cargo.toml\n")
            subprocess.run(["git", "add", ".gitignore"], cwd=self.base, check=True, capture_output=True)
        subprocess.run(["git", "commit", "--quiet", "-m", "fixture"], cwd=self.base, check=True, capture_output=True)

    def test_nested_tracked_cargo_verification_accepts_clean_checkpoint(self):
        """AC-05 regression: nested tracked Cargo project retains clean SHA gate."""
        self.init_parent_repository()
        for shell in ("bash", "pwsh"):
            with self.subTest(shell=shell):
                result, calls = self.invoke(shell, script="verify")
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(self.stages(calls), ["fmt", "clippy", "test", "test", "test"])
                self.assertIn("TOOL PASS", result.stdout)

    def test_nested_untracked_cargo_verification_rejects(self):
        """AC-05 regression: ignored/untracked manifest cannot relax root validation."""
        self.init_parent_repository(tracked_manifest=False)
        for shell in ("bash", "pwsh"):
            with self.subTest(shell=shell):
                result, calls = self.invoke(shell, script="verify")
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(calls, [])

    def test_nested_missing_cargo_verification_rejects(self):
        """AC-05 regression: arbitrary nested script directories remain rejected."""
        (self.root / "Cargo.toml").unlink()
        self.init_parent_repository()
        for shell in ("bash", "pwsh"):
            with self.subTest(shell=shell):
                result, calls = self.invoke(shell, script="verify")
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(calls, [])

    def test_dirty_sibling_verification_rejects(self):
        """AC-05 regression: cleanliness is checked across the parent repository."""
        self.init_parent_repository()
        (self.base / "dirty sibling.txt").write_text("dirty", encoding="utf-8")
        for shell in ("bash", "pwsh"):
            with self.subTest(shell=shell):
                result, calls = self.invoke(shell, script="verify")
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(calls, [])


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=SOURCE)
    options, remainder = parser.parse_known_args()
    SOURCE = options.source.resolve()
    unittest.main(argv=[sys.argv[0], *remainder], verbosity=2)
