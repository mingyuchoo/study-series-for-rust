# Independent Android verification

Spec: ANDROID-01 version 1. Builder `/root/builder`; verifier `/root/verifier`; independent reviewer `/root/reviewer`; coordinator `/root`. Date: 2026-10-03. Product/config were read-only to the verifier; tests and fixtures were independently authored.

Frozen source identity: `source-manifest.sha256`, SHA256 `7837fb850a062d64b087ca74865ffbfd050a9b24f7c00e92c5b555d1d99a4cb9`. Build outputs, tool caches, local.properties and verification reports are excluded. This scoped identity replaces a clean Git checkpoint because the workspace contains the user's preexisting desktop relocation. No staging or commits were performed.

## Current result

Final-12 automated verification PASS, exit 0, on dedicated emulator-5560 / Stillnote_Verification_Final, API 36: 14 UI tests passed, zero failures/errors/skips; 23 JVM cases (cached unchanged inputs), format, lint, Rust compatibility and APK build passed. All 37 frozen files were independently rehashed before and after the run, zero mismatches. Full acceptance remains incomplete for actual Korean IME, spoken TalkBack and physical-device manual checks. Final-08 passed all checks and all 14 UI tests. Final-09 failed after external Android Studio deployment took its test activity out of the foreground. Subsequent full and targeted failures identified exact test viewport/IME/materialization problems; evidence and corrections are preserved below. The product is unchanged; subsequent edits make fixture Cargo.lock eligible for inclusion and correct the test harness. Failed invocations are not classified as PASS.

## Reproduction and isolation

Execution directory: `C:/Users/mingy/github/mingyuchoo/study-series-for-rust/GUI/gpui/still-note/android-app`.

JDK: `C:/Users/mingy/.codex/tools/stillnote-android/jdk17/jdk-17.0.20.1+1`. SDK: `C:/Users/mingy/AppData/Local/Android/Sdk`. Set JAVA_HOME, ANDROID_HOME and platform-tools on PATH; set ANDROID_SERIAL to the assigned dedicated API 36 emulator. Run `scripts/verify.ps1` from android-app, or invoke it from the workspace root. Its serial/qemu/API checks reject physical devices and other API levels. No user's journal or physical device was used. Instrumentation installs the test package, launches an empty ComponentActivity test host, and uses UUID-named isolated cache files with cleanup. Real MainActivity observations were made separately by the coordinator.

Use an exclusively owned, unlocked test emulator: wait for `sys.boot_completed=1` and API 36/qemu confirmation, then avoid Android Studio deployments or other foreground interactions for the whole run. Final environment: emulator-5560 / Stillnote_Verification_Final, physical 1080x2400 / density 420. Enable its software keyboard even when the host keyboard is connected before running tests:

```powershell
adb -s $env:ANDROID_SERIAL shell getprop sys.boot_completed
adb -s $env:ANDROID_SERIAL shell settings put secure show_ime_with_hard_keyboard 1
adb -s $env:ANDROID_SERIAL shell settings get secure show_ime_with_hard_keyboard
```

Expected outputs are boot 1 and keyboard 1. Apply this only to the dedicated disposable test emulator; final-09 demonstrates why sharing the foreground with another deployment invalidates UI evidence.

The mandatory sequential commands are:

```text
gradlew.bat spotlessCheck lintDebug testDebugUnitTest assembleDebug
cargo run --locked --manifest-path app/src/test/rust-fixture/Cargo.toml -- validate app/build/compatibility-roundtrip.json
gradlew.bat connectedDebugAndroidTest
```

## Test coverage and evidence

Final evidence: `.artifacts/verification/final-12/verify.log`, `android-results/` (14-case XML and each test's logcat), `jvm-results/` (Journal 8 + Store 15 XML), `lint-results-debug.xml`, and `source-equality.txt`. The corrected materialization search diagnostic is separately preserved under `.artifacts/verification/search-materialization-diagnostic/`: one executed case, zero failures/errors/skips, exit 0, before the final full run. Full instrumentation completed in 4m 5s. No further checks were rerun after this success.

The JVM suite contains 8 domain tests and 15 storage/compatibility tests. XML records 23 tests, zero failures/errors/skips. The latest invocations reused Gradle up-to-date results from previously executed identical JVM inputs; this is cached evidence, not a claim that JVM cases executed again on every invocation. Instrumentation executes 14 cases: 12 flows plus two forced-size/font layout journeys. Each required category contains executed tests; none are ignored or skipped.

| AC | Independent evidence | Result / limit |
|---|---|---|
| AC-01 | Blank load writes no seeds; empty launch UI; APK assembly; Gradle min/target/compile SDK36 and manifest without network permission | Build/JVM PASS; UI PASS |
| AC-02 | JournalTest Unicode/kinds/whitespace/symbols and status restrictions; UI capture/edit/important/complete/reopen/cancel with restart | JVM PASS; UI PASS |
| AC-03 | Fixed leap/year0001/9999/date/month boundary expectations; real pointer calendar day29, counts, previous/next, Future capture | JVM PASS; UI PASS; coordinator actual MainActivity calendar pointer PASS |
| AC-04 | Trim/uniqueness and log visibility; long collections, automatic created-collection selection, explicit selection, Index and origin navigation | JVM PASS; UI PASS |
| AC-05 | Global Unicode/case search across logs, All/Open/Complete with notes/events, clear and result-origin navigation | JVM PASS; targeted search diagnostic 1/1 PASS; UI PASS |
| AC-06 | Reciprocal IDs, copied importance, immutable frozen original, permitted targets, rejected same/closed/collection locations, invalid links/cycles; UI links both directions | JVM PASS; UI PASS |
| AC-07 | Fixture generated using actual desktop model.rs serde derives; exact semantic Kotlin decode/encode/durable restart; actual Rust model validates Kotlin output | PASS; no desktop product edits |
| AC-08 | Backup equals previous durable snapshot; injected read/backup/replace failures and real blocked backup path; external edit/deletion; 12 serialized concurrent writes; corrupt JSON/type/UUID/duplicate escaped keys/UTF8/surrogates/depth protects original bytes and model; UI draft/conflict/recovery | JVM PASS; UI PASS |
| AC-09 | Separate settings/defaults/enums/corruption/conflict/failure preservation; live Korean/English, System/Light/Dark and journal usable with corrupt settings | JVM PASS; UI PASS; palettes/system icons observed separately |
| AC-10 | Compose saved-instance restoration plus a genuinely new ViewModel and SavedStateHandle preserve draft/date/log/filter/search/edit/migration; immediate selection range2..7 restored; synthetic Hangul InputConnection composition then Done | UI PASS; actual Korean keyboard and OS process-kill observation unverified |
| AC-11 | Real pointer capture/back/dialog actions; >=48dp primary target bounds; 320x480 and1000x360 at font1.5; long text/18collections, both locales/palettes; native keyboard window waits | UI PASS; actual MainActivity native Latin Gboard capture and tablet screenshot PASS; spoken TalkBack/physical device pending |
| AC-12 | Wrapper, pinned dependencies, readme recovery/build/run, font notices, failure-propagating serial-guarded scripts; Cargo.lock ignore exception confirmed using git check-ignore | Tooling inspection PASS; independent review remains a separate gate |

Normal flow tests keep the native keyboard active. Forced-size tests invoke public accessibility SetText without forcing a real phone keyboard into an artificial viewport; pointer capture/edit/save assertions remain. Only artificial-layout and synthetic InputConnection hosts exclude native IME, so the latter checks Android composition protocol rather than a real Korean Gboard. Semantics/text action setup is distinct from real pointer button interactions.

Lint reports zero errors and 11 warnings: DataExtractionRules 1, GradleDependency 5, MissingApplicationIcon 1, NewerVersionAvailable 3, OldTargetApi 1. SDK36 is the requested target. No lint warning suppression was added by the verifier. These warnings remain visible in the archived report.

APK: `app/build/outputs/apk/debug/app-debug.apk`, 32,881,430 bytes; SHA256 `b2e416026fee3baa0bcd8bae452ba94486393e4dc940ca17bf055342596888c3`.

## Preserved failures and corrections

Logs are retained under `.artifacts/verification/`; passing retries do not erase failed runs. Initial setup problems (relative log destination and missing ANDROID_HOME) remain in final-01. Review-led product corrections addressed dialog palettes/scrolling, strict duplicated JSON keys, UTF16/depth validation and runtime system-bar appearance before later freezes.

Earlier UI failures exposed test harness issues: lazy descendants not yet composed; focus blur naturally collapsing selection before an incorrectly placed assertion; fake window/native keyboard mismatch; competing native keyboard during synthetic InputConnection; pointer misses during native IME/window animation; and a collection creation flow that correctly closed its menu before the test attempted a second menu-item click. Assertions were retained and the harness now waits for native-window idle before visibility/real-pointer actions. Final-03 through final-07 preserve original failures and logcat/XML. Final-08 executed all 14 successfully. The initial targeted search diagnostic separately passed 1/1 before that full run.

Final-09 executed 14, failures 1, errors 0, skipped 0, exit 1. At 12:34:46 logcat records another project's `com.jetbrains.sample.app` force-stop/deploy; at 12:34:48 studio.deploy starts that app. Our ComponentActivity becomes PAUSED, invisible, then STOPPED. Compact layout's first settings-menu interaction fails with `No compose hierarchies found` at AdaptiveLayoutTest.kt110. Full XML/logcat and extracted evidence are preserved in `.artifacts/verification/final-09/android-results` and `foreground-interference.txt`. No other application's files/processes were altered to address this interference.

Final-10 on the newly created exclusive emulator-5560 executed 14, failures 3, errors 0, skipped 0, exit 1. Calendar pointer correctly changed the date/log but the immediate date assertion found zero nodes because the date editor was outside LazyColumn composition after navigation. That assertion now scrolls the date editor into view and retains the exact expected text. Expanded day1 visibility now uses the same native-idle/recomputed scroll helper as the normal flows; targeted diagnostic recorded day bounds[284,272,379,324] inside viewport[267,128,1045,324], root[0,0,1075,387]. Whitespace capture's physical pointer did not invoke its callback; native IME visibility/resizing had not been explicitly awaited. A timing problem is inferred from this missing readiness and the successful targeted result after adding that wait. Normal Activity text entry now waits for the real IME to be visible and its insets stable for 300ms before scrolling/touch; dialog windows and synthetic/artificial hosts are excluded. Capture bounds/root/IME and screenshots are recorded without removing any callback, text, status or visibility assertions. The targeted three-case invocation on this revised frozen source passed 3/3, zero errors/skips, exit 0. Original final-10 logs/XML plus diagnostic logs/XML remain separate.

Final-11 executed 14, failures 3, errors 0, skipped 0, exit 1. Two failures were caused by the newly added IME-readiness precondition: semantic text replacement does not guarantee a dismissed native keyboard is shown again. Normal input now actually taps its text field, awaits native IME readiness, then replaces text. On private emulator-5560 only, show_ime_with_hard_keyboard was changed 0→1 to deliberately exercise the software keyboard (before/after evidence in final-11/private-ime-setting.txt). The third failure was search entry visibility; adding more idle waits did not resolve it. Four-case diagnostics passed the three other flows and retained the search failure.

A diagnostic-only search invocation captured LayoutInfo placed=false, attached=true, cached nonzero bounds inside the viewport; all ancestors and the native root were visible. The persisted screenshot confirms the result card was not rendered: `.artifacts/verification/search-placement-diagnostic/images/verification-search-40dacf0f-66b1-495d-99e1-92f9da6129ab.png`. Compose 1.9.4 bytecode independently confirms isDisplayed rejects an unplaced node and Lazy performScrollToNode ignores unplaced nodes when locating matches. The helper previously checked only tag existence and performed ordinary ScrollTo against cached coordinates. It now materializes the Lazy item when its tag is absent OR its node/ancestor is unplaced, then retains the original display assertion. This is a harness correction proven by placement flags and screenshot, not an assumption that nonzero cached bounds mean visibility. Diagnostic bytecode and failed targeted XML/logcat are retained separately.

## Manual limits

See `manual-observations.md` for the coordinator's independent real MainActivity calendar, persisted capture with native Latin Gboard still visible, and expanded Korean/Dark screenshot with readable system icons. Those product observations remain relevant because later changes affect tests and the ignore exception only. Screenshots support observed layouts, not every device combination. The API 36 image's Gboard has no Korean subtype. Actual Korean IME, spoken TalkBack and physical-device behavior remain unverified; automated Hangul protocol and accessibility semantics do not replace those checks. This verification report does not authorize deployment or claim the independent reviewer gate has passed.
