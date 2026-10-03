"""Recording-command release checks; real build/install evidence is separate."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys
import unittest
import zipfile

sys.dont_write_bytecode = True
SOURCE = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('run_checks', SOURCE / 'tests/run-scripts/test_run_scripts.py')
run = importlib.util.module_from_spec(spec)
spec.loader.exec_module(run)
HOST = 'aarch64-pc-windows-msvc'
RECORDER = r'''
import json, os, sys
from pathlib import Path
name, *args = sys.argv[1:]
root = Path(os.environ['FIXTURE_ROOT'])
with open(os.environ['CARGO_CALL_LOG'], 'a', encoding='utf-8') as f:
    f.write(json.dumps({'tool': name, 'cwd': os.getcwd(), 'args': args}) + '\n')
stage = args[0] if args else ''
if name == 'cargo' and stage == 'test': stage = 'doc' if '--doc' in args else 'test'
if name == 'iscc': stage = 'iscc-version' if '/?' in args else 'installer'
if '--version' in args: stage += '-version'
if stage == os.environ.get('FAIL_STAGE'): sys.exit(37)
if name == 'rustc': print('rustc 1.90.0\nhost: aarch64-pc-windows-msvc')
if name == 'rustup': print('aarch64-pc-windows-msvc\nx86_64-pc-windows-msvc')
if name == 'cargo' and stage == 'metadata':
    print(json.dumps({'packages': [{'name': 'stillnote', 'version': '0.1.0', 'manifest_path': str(root / 'Cargo.toml'), 'targets': [{'name': 'stillnote', 'kind': ['bin']}]}], 'target_directory': str(root / 'custom target with spaces')}))
if name == 'cargo' and stage == 'build' and os.environ.get('MISSING') != 'binary':
    path = root / 'custom target with spaces' / args[args.index('--target') + 1] / 'release/stillnote.exe'
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b'MZ independently built fixture')
if name == 'iscc' and stage == 'iscc-version': print('Inno Setup 6 Command-Line Compiler')
if name == 'iscc' and stage == 'installer' and os.environ.get('MISSING') != 'installer':
    defs = dict(a[2:].split('=', 1) for a in args if a.startswith('/D'))
    (Path(defs['OutputDir']) / (defs['OutputBaseFilename'] + '.exe')).write_bytes(b'MZ installer fixture')
'''

class ReleaseScripts(unittest.TestCase):
    setUp = run.RunScripts.setUp
    invoke = run.RunScripts.invoke

    def fixture(self):
        for name in ('packaging', 'assets', 'licenses'):
            if (SOURCE / name).exists(): shutil.copytree(SOURCE / name, self.root / name)
        shutil.copy2(SOURCE / 'README.md', self.root / 'README.md')
        recorder = self.bin / 'release_record.py'
        recorder.write_text(RECORDER, encoding='utf-8')
        for name in ('cargo', 'rustc', 'rustup', 'iscc'):
            (self.bin / (name + '.cmd')).write_text('@echo off\r\n"' + sys.executable + '" "' + str(recorder) + '" ' + name + ' %*\r\n', encoding='utf-8')
        before = os.environ.copy()
        self.addCleanup(lambda: (os.environ.clear(), os.environ.update(before)))
        os.environ.update(FIXTURE_ROOT=str(self.root), ISCC_PATH=str(self.bin / 'iscc.cmd'))
        os.environ.pop('MISSING', None)

    def release(self, shell, args=(), failure=None):
        return self.invoke(shell, args, failure=failure, script='release')

    def pipeline(self, calls):
        stages = []
        for c in calls:
            a = c['args']
            if c['tool'] == 'iscc' and '/?' not in a: stages.append('installer')
            elif c['tool'] == 'cargo' and a[0] in ('fmt', 'clippy', 'test', 'build') and '--version' not in a:
                stages.append('doc' if '--doc' in a else a[0])
        return stages

    def test_order_paths_targets_and_artifacts(self):
        """AC01/02/04: both entrypoints, spaces/foreign cwd, metadata paths, unique ZIP/EXE/hash."""
        self.fixture()
        self.assertIn('MinVersion=10.0', (self.root / 'packaging/stillnote.iss').read_text())
        for shell in ('pwsh', 'bash'):
            result, calls = self.release(shell, ['--target', 'x86_64-pc-windows-msvc'])
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            installer_lines = [line.removeprefix('Installer: ') for line in result.stdout.splitlines() if line.startswith('Installer: ')]
            self.assertEqual(len(installer_lines), 1, result.stdout)
            installer_path = Path(installer_lines[0])
            self.assertTrue(installer_path.is_absolute(), installer_lines[0])
            self.assertTrue(installer_path.is_file(), installer_lines[0])
            self.assertEqual(installer_path.name, 'stillnote-0.1.0-x64-setup.exe')
            self.assertIn('Release complete:', result.stdout)
            self.assertEqual(self.pipeline(calls), ['fmt', 'clippy', 'test', 'doc', 'build', 'installer'])
            for c in calls:
                a = c['args']
                self.assertEqual(Path(c['cwd']).resolve(), self.root.resolve())
                if c['tool'] == 'cargo' and a[0] in ('metadata', 'clippy', 'test', 'build') and '--version' not in a:
                    self.assertIn('--locked', a)
                    if a[0] in ('clippy', 'test'):
                        self.assertIn('test-support', a)
                        self.assertEqual(a[a.index('--target') + 1], HOST)
                        if '--doc' not in a: self.assertIn('--all-targets', a)
                    elif a[0] == 'build':
                        self.assertIn('--release', a)
                        self.assertNotIn('test-support', a)
                        self.assertEqual(a[a.index('--target') + 1], 'x86_64-pc-windows-msvc')
        dirs = list((self.root / '.artifacts/releases').iterdir())
        self.assertEqual(len(dirs), 2)
        for d in dirs:
            archive, = d.glob('*-0.1.0-x64.zip')
            installer, = d.glob('*-0.1.0-x64-setup.exe')
            with zipfile.ZipFile(archive) as z:
                for suffix in ('stillnote.exe', 'README.md', 'GPUI-LICENSE-APACHE', 'LICENSE-Pretendard.txt'):
                    self.assertTrue(any(n.endswith(suffix) for n in z.namelist()), z.namelist())
                binary = next(n for n in z.namelist() if n.endswith('stillnote.exe'))
                self.assertEqual(z.read(binary), b'MZ independently built fixture')
            sums = (d / 'SHA256SUMS.txt').read_text(encoding='utf-8-sig')
            for f in (archive, installer):
                self.assertIn(hashlib.sha256(f.read_bytes()).hexdigest().lower(), sums.lower())
                self.assertIn(f.name, sums)

    def test_preflight_options_and_missing_compiler(self):
        """AC02: help inert; wrong options/unsupported target/missing compiler precede format."""
        self.fixture()
        for shell in ('pwsh', 'bash'):
            for args in (['--help'], ['--bad'], ['--target'], ['--target', 'x86_64-unknown-linux-gnu'], ['two words']):
                result, calls = self.release(shell, args)
                self.assertEqual(result.returncode, 0 if args == ['--help'] else 2, result.stdout + result.stderr)
                self.assertEqual(self.pipeline(calls), [])
                if args == ['--help']: self.assertEqual(calls, [])
            os.environ['ISCC_PATH'] = str(self.bin / 'missing compiler.exe')
            result, calls = self.release(shell)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(self.pipeline(calls), [])
            os.environ['ISCC_PATH'] = str(self.bin / 'iscc.cmd')
        self.assertFalse((self.root / '.artifacts/releases').exists())

    def test_native_failure_and_missing_outputs(self):
        """AC03: every stage preserves exit37; absent generated binary/installer never success."""
        self.fixture()
        stages = ['fmt', 'clippy', 'test', 'doc', 'build', 'installer']
        for shell in ('pwsh', 'bash'):
            for i, stage in enumerate(stages):
                result, calls = self.release(shell, failure=stage)
                self.assertEqual(result.returncode, 37, result.stdout + result.stderr)
                self.assertNotIn('Installer:', result.stdout)
                self.assertNotIn('Release complete:', result.stdout)
                self.assertEqual(self.pipeline(calls), stages[:i + 1])
            for stage in ('metadata', 'fmt-version', 'clippy-version'):
                result, calls = self.release(shell, failure=stage)
                self.assertEqual(result.returncode, 37, result.stdout + result.stderr)
                self.assertNotIn('Installer:', result.stdout)
                self.assertNotIn('Release complete:', result.stdout)
                self.assertEqual(self.pipeline(calls), [])
            for missing in ('binary', 'installer'):
                shutil.rmtree(self.root / 'custom target with spaces', ignore_errors=True)
                os.environ['MISSING'] = missing
                result, calls = self.release(shell)
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertNotIn('Installer:', result.stdout)
                self.assertNotIn('Release complete:', result.stdout)
                if missing == 'binary': self.assertNotIn('installer', self.pipeline(calls))
            os.environ.pop('MISSING', None)
        self.assertEqual(list(self.root.glob('.artifacts/releases/*/SHA256SUMS.txt')), [])

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, default=SOURCE)
    options, remainder = parser.parse_known_args()
    SOURCE = options.source.resolve()
    run.SOURCE = SOURCE
    unittest.main(argv=[sys.argv[0], *remainder], verbosity=2)
