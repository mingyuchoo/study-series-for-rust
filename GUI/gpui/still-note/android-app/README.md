# Stillnote Android

Native Kotlin / Jetpack Compose counterpart to `../desktop-app`. Requires Android
16 (API 36) or newer. Blank first launch, offline, no account or network permission.
Daily, Monthly, Future, collections, index, global search, status filters,
importance, editing, cancellation, completion/reopen and linked task migration are
implemented. Korean/System are defaults; language and theme update immediately.
The bundled Pretendard 1.3.9 fonts retain their SIL OFL in LICENSE-Pretendard.txt.

The launcher uses an original notebook icon with task/event/note marks, adaptive
shape support and a monochrome layer for themed icons. See
[icon assets and generation notes](docs/APP-ICON.md).

## Architecture

The Kotlin/JVM `core` module contains immutable domain rules, explicit commands,
repository ports and use cases. The Android `app` module supplies file/JSON adapters,
Compose UI, ViewModel scheduling and dependency composition. See
[architecture and refactoring notes](docs/ARCHITECTURE.md) for dependency rules,
pure/effect boundaries and preserved persistence policies. Run
`./kotlin.bat test -m core` for the core tests without an emulator or Android SDK.

The ViewModel depends on the `JournalUseCases` inbound port. `StillnoteRoute.kt`
connects Android lifecycle, saved state and clock effects to `StillnoteScreen.kt`,
which receives screen state and event/memory contracts. Search and index queries
are pure core use cases; `ProtectedFileStore` owns shared file protection and
durable-baseline handling. `./kotlin.bat run -m tooling -- architecture` checks
inward dependency rules and is included in the verification scripts.

The Android module uses `app/src`, `app/res`, `app/test`, and `app/testResources`;
UI tests use `app/instrumentedTest`. The JVM core retains `src/main/kotlin` and
`src/test/kotlin` through the Maven-like layout. Package names remain
`app.stillnote` and its subpackages.

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

CLI builds are verified independently of an IDE. For IDE integration, follow
the official [IntelliJ IDEA and Kotlin Toolchain plugin setup](https://kotlin-toolchain.org/latest/getting-started/ide-setup/),
including its Android plugin instructions. The former Gradle import is no longer
the project entry point.

### 기존 IDE의 Gradle 연결 오류 해결

이 프로젝트의 빌드 진입점은 JetBrains **Kotlin Toolchain**이다.
`Directory ... does not contain a Gradle build` 오류는 IDE가 이전 Gradle
프로젝트 연결을 유지하거나 루트에서 `gradle` 명령을 실행할 때 발생한다.
빌드는 프로젝트 루트의 PowerShell에서 `./kotlin.bat build`로 실행한다.

IDE에서는 다음 순서로 다시 연결한다.

1. Gradle 도구 창에 남은 `android-app` 연결을 **Unlink Gradle Project**로 해제한다.
2. Kotlin Toolchain 플러그인과 Android 플러그인을 설치한 IntelliJ IDEA에서
   `project.yaml`이 있는 이 디렉터리를 다시 연다.
3. Kotlin Toolchain으로 프로젝트를 가져오고 동기화한다.

IDE가 연결 해제를 제공하지 않는다면 IDE를 닫고 `.idea/gradle.xml`의
`GradleSettings` 컴포넌트에서 이 루트를 가리키는 `GradleProjectSettings`를
제거한 뒤 다시 연다. `.idea`는 로컬 설정이므로 다른 체크아웃에서도 이전
연결을 별도로 해제해야 한다. 루트에 `gradle init`을 실행하거나 Gradle
settings/build 파일을 추가하지 않는다. Android APK 패키징에 필요한
Gradle/AGP 호출은 Kotlin Toolchain이 내부에서 관리한다.

### Android Studio 실행 버튼이 포맷 검사만 수행하는 경우

`kotlin run --module tooling`과 `Kotlin formatting checked`가 출력되면
실행 대상으로 검증 유틸리티인 `tooling`이 선택되어 있다. 인자 없는
`tooling` 실행은 포맷 검사 후 종료하는 정상 동작이다.

1. 상단 실행 설정에서 저장소에 포함된 **Stillnote Android**를 선택한다.
2. 실행 장치에서 Android API 36 이상 에뮬레이터를 선택한다.
3. **Run** 또는 `Shift+F10`으로 실행한다.

공유 설정은 `.run/Stillnote_Android.run.xml`에 있으며 Android App 타입으로
`app.main` 모듈의 기본 Activity를 실행한다. 실행 전
`.run/Stillnote_Build.run.xml`의 **Stillnote build** 설정으로 `kotlin build`를
수행하므로 변경된 코드도 APK에 반영된다. 빌드 실패 시 앱 실행도 중단한다.
설정이 보이지 않으면 **File → Reload All from Disk**로 새로 고침한다.
**Run → Edit Configurations**에서 **Before launch**의
**Run Another Configuration: Stillnote build** 작업을 유지한다.
Kotlin CLI의 `tooling` 설정은 검증용으로 사용한다.

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
builds do not support `run-as`; uninstalling clears internal data.

## Verification

```powershell
./scripts/verify.ps1
```

Or `bash scripts/verify.sh`. Requires Rust for the independent desktop-model JSON
roundtrip harness and an isolated Android 16 (API 36) emulator for Compose interaction
tests. Set `ANDROID_SERIAL` to its `emulator-NNNN` serial and expose Android SDK
platform-tools on `PATH`. Enable the software keyboard on that isolated emulator
with `adb -s emulator-NNNN shell settings put secure show_ime_with_hard_keyboard 1`.
The scripts reject physical devices and non-API-36 emulators. The UI adapter keeps
the selected emulator awake and dismisses its keyguard before testing; `ANDROID_HOME` must
point to the installed SDK. All commands propagate failures. Use
`./scripts/format.ps1 -Write` or `bash scripts/format.sh --write` to format Kotlin.
Check reports under `build/android-checks/build/reports` and dated evidence under
`docs/verification`. Physical-device keyboard composition and TalkBack must be
checked on an actual device; automated tests are not evidence of those manual
checks.

The `tooling` module is the verification utility. Running
`./kotlin.bat run --module tooling` without arguments defaults to a read-only Kotlin
formatting check. Pass `-- format --write`, `-- architecture`, `-- lint`, or `-- ui` for the other
actions. To launch the Android application, run `./kotlin.bat run -m app`
with `--device-id emulator-NNNN` as shown above.

`./kotlin.bat test` runs 35 core/app JVM tests, with AGP's mockable
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
