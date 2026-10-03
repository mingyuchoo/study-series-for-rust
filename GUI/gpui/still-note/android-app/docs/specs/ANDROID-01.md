# ANDROID-01 — Stillnote Android parity

Version: 1. Status: READY. Spec owner: /root.

## Scope

Implement a native Kotlin / Jetpack Compose Android 16 (min/target/compile SDK 36)
application in android-app. desktop-app is read-only and its current source,
especially model.rs, store.rs, settings.rs, theme.rs and ui.rs, is authoritative.
No accounts, network, sync, seeded journals or destructive record deletion.
Use bundled Pretendard 400/500/600/700 and preserve its OFL notice. Preserve actual
dark/light palettes, flat styling, 8dp controls, 12dp cards, yellow primary/focus.
Adapt desktop layout for phones, tablets, short windows, keyboard and font scaling.

## Acceptance criteria

| ID | Observable requirement | Evidence |
|---|---|---|
| AC-01 | Gradle-wrapper build of native Compose APK, API 36 minimum/target; blank first launch, no network permission or seed writes | build, manifest, UI test |
| AC-02 | Daily Task/Event/Note capture, whitespace rejection, Unicode, edit, importance, cancel, task complete/reopen, correct symbols | unit + UI |
| AC-03 | Date range 0001–9999, leap years, previous/next/today; Monthly calendar date tiles/counts opening Daily; Monthly/Future month navigation/capture | unit + UI |
| AC-04 | Trimmed unique collection creation/selection/capture; index opens recorded day/month/log and all collections | unit + UI |
| AC-05 | Global lowercased substring search and All/Open/Complete filtering; Open includes open notes/events; clear restores selected log, result opens original location | unit + UI |
| AC-06 | Open tasks migrate to different Daily/Monthly/Future location only; reciprocal IDs, open target, copied importance, frozen original with > or <, links navigate; validate missing/cyclic links | unit + integration + UI |
| AC-07 | Desktop serde version-1 JSON compatibility including externally tagged Log::Collection, ISO dates, UUID and snake_case migrated fields; durable restart | fixture roundtrip + integration |
| AC-08 | Clone/validate/persist before publishing state; atomic replacement and previous durable backup; baseline conflict checks and serialized writes; failures preserve draft/model/original bytes; corrupt load blocks mutation | integration + UI |
| AC-09 | Korean/English and System/Light/Dark selections, Korean/System defaults; live system theme; separate settings persistence, corrupt settings preserved and journal still usable; setting errors localized | unit + integration + UI |
| AC-10 | Theme/language/rotation changes preserve draft, selection, search, date/log/filter/edit/migration; small saved state for system process recreation; journal persisted separately | UI + manual IME |
| AC-11 | Edge-to-edge insets, IME-safe capture, supported back dispatch; reachable touch targets >=48dp, scrolling long text/collections, compact/expanded layout, both locales/themes, TalkBack descriptions | UI + emulator screenshots/manual |
| AC-12 | README build/run/data/recovery instructions, reproducible verification scripts with failures propagated, dependency versions pinned, font notices | tooling + review |

## Architecture / API agreement

Package app.stillnote. Domain has immutable Journal/Entry/Collection with Kotlin
Kind { Task, Event, Note }, Status { Open, Complete, Cancelled, Migrated, Scheduled },
Filter { All, Open, Complete }, sealed Log Daily/Monthly/Future/Collection(UUID).
Builder must send exact domain/store APIs to verifier promptly before tests are written.
UI uses ViewModel/StateFlow, storage runs off main thread, one repository writer.
Settings preserve desktop JSON language korean/english and theme system/light/dark.
Internal files journal.json, journal.json.bak, settings.json; no automatic recovery/reset.
Explicit recovery guidance must be available in README and corrupt-data UI.

## Ownership and verification

Builder owns android-app product/config/resources/scripts/README, excludes
app/src/test, app/src/androidTest, test fixtures, docs/specs, docs/verification.
Verifier owns those test subtrees and docs/verification/test-report.md.
Reviewer owns docs/verification/review-report.md only.
Orchestrator owns spec, coordination reports and isolated local tool provisioning.
Read desktop-app/AGENTS.md and assigned .agents role as parity workflow reference;
Android commands replace Rust-specific commands. No edits to desktop-app or unrelated files.
No commit/staging of user's ongoing desktop relocation or unrelated changes.
Use scoped source SHA256 manifest to identify frozen Android tree because repository
contains preexisting relocation changes. Verification and review must name same manifest.
Mandatory checks: format check, Android lint, JVM domain/file tests, build APK,
instrumented Compose user-flow tests on available Android 16 emulator. Never claim
manual real-device/IME checks or unavailable emulator checks passed.
