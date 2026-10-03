$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
if ($env:ANDROID_SERIAL -notmatch '^emulator-\d+$') { throw 'Set ANDROID_SERIAL to the isolated API 36 emulator.' }
$qemu = & adb -s $env:ANDROID_SERIAL shell getprop ro.kernel.qemu
$sdk = & adb -s $env:ANDROID_SERIAL shell getprop ro.build.version.sdk
if ($qemu.Trim() -ne '1' -or $sdk.Trim() -ne '36') { throw 'Verification requires the isolated Android 16 emulator.' }
& ./kotlin.bat run -m tooling -- format
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& ./kotlin.bat run -m tooling -- architecture
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& ./kotlin.bat build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& ./kotlin.bat test
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& cargo run --locked --manifest-path scripts/rust-fixture/Cargo.toml -- validate build/compatibility-roundtrip.json
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& ./kotlin.bat run -m tooling -- lint
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& ./kotlin.bat run -m tooling -- ui
exit $LASTEXITCODE
