# Android implementation handoff

> Historical verification snapshot. Results, source hashes and paths below apply only to that run.
> Current structure and commands: [architecture](../ARCHITECTURE.md), [README](../../README.md).

Spec: ANDROID-01 version 1. Date: 2026-10-03.
Source manifest SHA256: 7837fb850a062d64b087ca74865ffbfd050a9b24f7c00e92c5b555d1d99a4cb9 (37 files).
Coordinator: /root. Builder: /root/builder. Independent verifier: /root/verifier. Independent reviewer: /root/reviewer.

## Delivered

Native Kotlin / Jetpack Compose application in android-app, minimum/target/compile Android16 API36. Daily/Monthly/Future logs, Task/Event/Note capture, status/importance/editing, linked task migration, collections, index, global search and filters; Korean/English and System/Light/Dark; Pretendard and desktop palette/control styling adapted for compact and expanded screens. Desktop version1 JSON interoperability, durable atomic writes, prior journal backup, protected corrupt-data handling and separate settings are implemented.

Debug APK: app/build/outputs/apk/debug/app-debug.apk.
APK SHA256: b2e416026fee3baa0bcd8bae452ba94486393e4dc940ca17bf055342596888c3

README contains build/run/data/recovery instructions. The verification report provides exact JDK/SDK/emulator/software-keyboard prerequisites. Gradle wrapper and Rust fixture Cargo.lock are eligible for Git inclusion. Existing desktop and unrelated workspace changes were not staged or committed.

## Final automated verification

Execution directory: android-app. Final invocation: scripts/verify.ps1, ANDROID_SERIAL=emulator-5560, isolated Stillnote_Verification_Final AVD, API36, exit0. Source hashes were independently checked against the same frozen manifest by verifier and coordinator.

| Check | Result |
|---|---|
| Spotless format check | PASS |
| Android lint | PASS: 0 errors, 11 unsuppressed warnings |
| JVM tests | PASS: 23, no failures/errors/skips; unchanged inputs allowed Gradle cache |
| Real desktop Rust model JSON roundtrip | PASS |
| Debug APK assembly | PASS |
| Instrumented Compose tests | PASS: 14, no failures/errors/skips |

Final evidence: .artifacts/verification/final-12/verify.log and preserved test XML.
See [independent test report](test-report.md) for AC01–12 evidence and [independent review](review-report.md) for findings and approval limits.

## Manual observations and remaining validation

Coordinator and reviewer inspected actual tablet dark layout/system bars and actual phone native Latin keyboard layout. Actual MainActivity pointer calendar navigation and native-keyboard pointer capture were confirmed, including persisted journal bytes. See [manual observations](manual-observations.md).

Physical Korean keyboard composition, spoken TalkBack, actual-device observations and actual OS process-kill restoration remain unverified. Implementation and automated checks are complete; the strict ANDROID-01 release gate remains pending/FAIL for these missing manual criteria. Do not describe the remaining manual checks as passed.

Historical failed attempts were preserved. An external Android Studio deployment interrupted final-09 on emulator5558; verification was moved to a newly isolated emulator without stopping other user work. Test-harness corrections retained the required assertions and real pointer paths, including proper Lazy item placement before scrolling/visibility checks. The final full run passed on the exact source above.
