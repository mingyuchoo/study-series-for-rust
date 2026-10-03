# Stillnote Android

Native Kotlin / Jetpack Compose counterpart to `../desktop-app`. Requires Android
16 (API 36) or newer. Blank first launch, offline, no account or network permission.
Daily, Monthly, Future, collections, index, global search, status filters,
importance, editing, cancellation, completion/reopen and linked task migration are
implemented. Korean/System are defaults; language and theme update immediately.
The bundled Pretendard 1.3.9 fonts retain their SIL OFL in LICENSE-Pretendard.txt.

## Architecture

The Kotlin/JVM `core` module contains immutable domain rules, explicit commands,
repository ports and use cases. The Android `app` module supplies file/JSON adapters,
Compose UI, ViewModel scheduling and dependency composition. See
[architecture and refactoring notes](docs/ARCHITECTURE.md) for dependency rules,
pure/effect boundaries and preserved persistence policies. Run
`./kotlin.bat test -m core` for the core tests without an emulator or Android SDK.

The Android module uses `app/src`, `app/res`, `app/test`, and `app/testResources`;
UI tests use `app/instrumentedTest`. The JVM core retains `src/main/kotlin` and
`src/test/kotlin` through the Maven-like layout. Package names remain
`app.stillnote` and its subpackages. Historical verification manifests retain the
source paths from their original runs.

## Build and run

Install Android SDK platform 36 and Build Tools 36.0.0, and set `ANDROID_HOME`.
The checked-in `kotlin`/`kotlin.bat` wrappers pin Kotlin Toolchain 0.13.0 (formerly
Amper) and verify its distribution checksum. They provision their own runtime;
the modules request JDK 17 and pin Kotlin 2.2.21 and the existing AndroidX versions
in YAML. A matching `JAVA_HOME` is reused; otherwise the toolchain provisions a
matching JDK. Verification adapters require `JAVA_HOME` pointing to JDK 17.

```powershell
./kotlin.bat build
adb -s emulator-NNNN install -r build/tasks/_app_buildAndroidDebug/gradle-project-debug.apk
# Or install and launch on an explicitly selected emulator:
./kotlin.bat run -m app --device-id emulator-NNNN
```

On Linux/macOS use `bash kotlin`. The app supports edge-to-edge, keyboard insets,
phone/tablet windows, rotation, font scaling and predictive system back. Expanded
windows show navigation in a sidebar; phone navigation remains directly reachable.
Tap Menu · Settings to create/select collections and change language/theme.
Monthly day tiles open the corresponding Daily log. Search covers every log;
clearing it restores the selected log. Open includes open events and notes.
Migration preserves the original, adds a linked open task and freezes the original.

## Data and recovery

Internal app files: `journal.json`, `journal.json.bak` (previous durable journal),
`settings.json`. JSON schema version 1 is compatible with the desktop model,
including ISO dates, UUIDs, externally tagged collection logs and migration links.
There is no automatic reset or backup restore. Corrupt journals block changes;
corrupt settings preserve the file while the journal remains usable. A failed save
preserves the draft and the published journal. Settings choices remain in the
current session even when settings persistence fails.

For a **debuggable build**, stop the app and use Android Studio Device Explorer or
`adb shell run-as app.stillnote` to copy the three files before recovery. For
example `adb exec-out run-as app.stillnote cat files/journal.json` exports current
bytes; preserve this output and the backup separately. Inspect both copies with a
JSON editor and restore a known valid desktop-compatible journal to
`files/journal.json` while the app is stopped, then restart. Never remove your only
copy. For invalid settings, first export the original and then correct the
`language` (`korean`/`english`) and `theme` (`system`/`light`/`dark`) fields. Release
internal data is accessible only through supported device backup/development
access; uninstalling clears it. Automatic OS backup is disabled to prevent
uncontrolled restore of stale journals.

## Verification

```powershell
./scripts/verify.ps1
```

Or `bash scripts/verify.sh`. Requires Rust for the independent desktop-model JSON
roundtrip harness and an isolated Android 16 (API 36) emulator for Compose interaction
tests. Set `ANDROID_SERIAL` to its `emulator-NNNN` serial and expose Android SDK
platform-tools on `PATH`. Enable the software keyboard on that isolated emulator
with `adb -s emulator-NNNN shell settings put secure show_ime_with_hard_keyboard 1`.
The scripts reject physical devices and non-API-36 emulators; `ANDROID_HOME` must
point to the installed SDK. All commands propagate failures. Use
`./scripts/format.ps1 -Write` or `bash scripts/format.sh --write` to format Kotlin.
Check reports under `build/android-checks/build/reports`, CLI logs under
`build/logs`, and evidence under
`docs/verification`. Physical-device keyboard composition and TalkBack must be
checked on an actual device; automated tests are not evidence of those manual
checks. Product code and tests have different owners under the repository's
independent verification contract.

`./kotlin.bat test` runs the existing 31 core/app JVM tests, with AGP's mockable
Android jar. The independent Rust harness validates `build/compatibility-roundtrip.json`.
`./kotlin.bat run -m tooling -- lint` checks the app sources. The `ui` command first
installs and launches the actual CLI APK on the selected emulator, then runs the
existing 14 Compose tests in `app.stillnote.verification`. This separate test host
uses the CLI's app/core JARs and app resources, adds the Compose test Activity,
and never recompiles production Kotlin. It does not modify production source or
the production manifest. UI results are under `build/android-checks/build/outputs/androidTest-results`.

Android packaging still delegates to Gradle/AGP inside Kotlin Toolchain. The
`tooling` JVM module temporarily generates an ignored Gradle verification project
for lint and instrumentation because 0.13.0 has no public commands for those checks.
The verification bridge pins Gradle 8.13/AGP 8.13.2 and reads app settings and
dependencies from `app/module.yaml`; app/core build definitions are YAML only.
Keep the wrappers and YAML files in Git. Generated projects, reports, and caches
stay under ignored `build` directories. See [migration evidence](docs/verification/toolchain-migration-report.md).
