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
`./gradlew.bat :core:test` for the core tests without an emulator.

Kotlin source roots use `src/main/kotlin` and `src/test/kotlin` in both modules;
Android UI tests use `app/src/androidTest/kotlin`. Package names remain
`app.stillnote` and its subpackages. Historical verification manifests retain the
source paths from their original runs.

## Build and run

Install JDK 17 and Android SDK platform 36. Set `JAVA_HOME` and `ANDROID_HOME`
(or create ignored `local.properties` with `sdk.dir`). Gradle wrapper downloads
Gradle 8.13; dependency versions are pinned in the Gradle files.

```powershell
./gradlew.bat assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

On Linux/macOS use `bash gradlew`. The app supports edge-to-edge, keyboard insets,
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
tests. Set `ANDROID_SERIAL` to its `emulator-NNNN` serial and expose Android SDK platform-tools on `PATH`. The scripts reject physical devices and non-API-36 emulators; `ANDROID_HOME` must point to the installed SDK. All commands propagate failures. Use `bash scripts/format.sh --write` only
after all writers stop. Check reports under `app/build/reports` and evidence under
`docs/verification`. Physical-device keyboard composition and TalkBack must be
checked on an actual device; automated tests are not evidence of those manual
checks. Product code and tests have different owners under the repository's
independent verification contract.
