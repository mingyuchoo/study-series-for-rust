# Independent review — ANDROID-01

Reviewer: `/root/reviewer`. Spec: ANDROID-01 version 1, READY. Product and tests were read-only to this reviewer; only this report was edited.

Reviewed frozen source manifest SHA256: `7837fb850a062d64b087ca74865ffbfd050a9b24f7c00e92c5b555d1d99a4cb9`. Independently rehashed all 37 listed files: zero mismatches. Reports and outputs are excluded from the manifest. The user's preexisting desktop relocation is not staged or committed; this scoped manifest is the agreed revision identity.

**Implementation review: PASS. Automated verification: PASS. Strict overall acceptance gate: FAIL / manual validation pending.** There are no unresolved implementation findings from this review. Actual Korean keyboard composition, spoken TalkBack and physical-device observation remain missing required evidence; automated protocol/semantics tests do not establish those observations.

## Evidence read directly

- `docs/specs/ANDROID-01.md`, desktop `AGENTS.md` and `.agents/reviewer-agent.md`.
- Authoritative desktop `model.rs`, `store.rs`, `settings.rs`, `theme.rs`, and `ui.rs`.
- Final Kotlin domain/store/strict JSON/ViewModel/activity/theme/UI, Gradle/wrapper/manifest/scripts/README/font notice, independent JVM/Compose tests and real desktop-model Rust fixture harness.
- `docs/verification/source-manifest.sha256`, `test-report.md`, and `manual-observations.md`.
- `.artifacts/verification/final-12/verify.log`, `source-equality.txt`, `lint-results-debug.xml`, JVM XML, and `android-results/TEST-Stillnote_Verification_Final(AVD) - 16-_app-.xml`.
- Independently viewed `.artifacts/screenshots/tablet-ko-dark.png` and `phone-native-keyboard-final.png`.

Final-12 log shows format checks, Android lint, JVM checks and APK build success, followed by real desktop Rust validation of all fixture values/migration links and successful API36 instrumentation. XML contains 8 domain plus 15 storage tests and 14 UI tests, with zero failures/errors/skips. JVM evidence was cached for unchanged inputs, as disclosed by the verifier; instrumentation executed on exclusive emulator-5560. Lint has zero errors and 11 visible warnings. No test assertions were removed to make failures pass, no tests were ignored, and the test suites contain meaningful behavioral expectations. Scripts propagate failures and guard against unintended physical-device runs. Cargo.lock is now eligible for normal source inclusion despite the parent ignore rule.

Earlier failed runs remain preserved. Lazy descendants required materialization when absent or unplaced; native-window/IME animation needed synchronization; collection creation correctly closes its menu; external Android Studio deployment interrupted another emulator. The final harness retains real pointer buttons and expected data/UI assertions. Artificial size/font tests use accessibility SetText and exclude a phone keyboard that cannot represent their synthetic viewport; the separate synthetic InputConnection test explicitly checks Hangul composition protocol. These limits are accurately recorded rather than reported as physical Korean keyboard verification.

## Findings resolved on final source

| Finding | Severity / AC | Resolution |
|---|---|---|
| R1 Desktop colors/action roles/dialog styling | P2 / AC-11 | Explicit container/tint palette roles, flat 12dp dialogs, primary filled save/create/migration actions, muted inactive navigation and ink controls. Inspected source and tablet screenshot. |
| R2 Short expanded sidebar/dialog reachability | P2 / AC-11 | Whole sidebar and dialog contents scroll; independent compact/expanded font1.5 long-text/18-collection journeys pass. |
| R3 Duplicate JSON keys silently collapse | P1 / AC-07/08 | Scanner rejects decoded duplicate keys including escaped names and nested objects before tree decoding. Corrupt load blocks writes and preserves bytes; regression tests pass. Real Rust harness independently reproduced rejection. |
| R4 Malformed UTF8/isolated surrogates/deep JSON | P1 / AC-07/08 | Strict UTF8 decoder, paired UTF16 validation for every key/string, and pre-parser nesting bound. Tests reject corrupted files and accept valid emoji. |
| R5 Explicit theme system-bar contrast | P2 / AC-11 | MainActivity applies status/navigation icon appearance whenever resolved theme changes. Actual dark tablet and light phone keyboard screenshots show readable icons. |

Early review also led to submitted-input snapshots/busy guards, full draft selection persistence, separate valid date state, localized weekdays, active dialog error display, Daily migration defaults, selected navigation and visible-record statistics. These were inspected on final source. Domain changes preserve frozen migration origins, reciprocal links, importance and state rules; settings failure remains isolated from journal mutation and applies the session preference without overwriting protected bytes. Persistent state publication occurs only after validation and durable save succeeds.

## AC assessment

| AC | Review result |
|---|---|
| AC-01 | PASS: native Compose API36-only APK, blank first launch, no network permission or seeds. |
| AC-02 | PASS: Unicode kinds, whitespace rejection, edit/importance/cancel/complete/reopen and symbols covered by real flows and domain assertions. |
| AC-03 | PASS: date bounds/leap/month transitions/calendar counts and Daily navigation. Actual MainActivity physical calendar tap separately observed by coordinator. |
| AC-04 | PASS: trimmed unique collections, auto/explicit selection, long names and index origin navigation. |
| AC-05 | PASS: global lowercased substring search, all/open/complete including notes/events, clearing and origin navigation. |
| AC-06 | PASS: open-task migration to permitted distinct locations, reciprocal IDs/copied importance/frozen source/link navigation and corrupt-link/cycle rejection. |
| AC-07 | PASS: version-1 serde shape, ISO dates/UUIDs/snake_case links and actual desktop-model fixture roundtrip. |
| AC-08 | PASS: validate/persist/publish ordering, previous backup, atomic replacement, baseline protection, serialized writes and failed/corrupt-load byte/model/draft preservation. |
| AC-09 | PASS for automated/source evidence: Korean/System defaults, bilingual errors, separate settings safety and palette selection; actual screenshot/system icons support theme behavior. |
| AC-10 | PARTIAL: saved instance and new ViewModel/SavedStateHandle tests preserve draft, selection, date/log/filter/search/edit/migration; synthetic Hangul protocol passes. Real Korean IME/physical-device and actual OS process-kill observation unverified. |
| AC-11 | PARTIAL: pointer flows/back/48dp checks/adaptive layouts and native Latin Gboard capture pass. Spoken TalkBack and physical-device observation unverified. |
| AC-12 | PASS: wrapper/pinned dependencies/failure-propagating verification/recovery README/font notice and Rust lockfile packaging. |

Coordinator manual observations and screenshots concern unchanged product code even where later manifests changed tests or packaging. The tablet screenshot has readable system icons, sidebar and main content; the light phone screenshot has the full Capture button and draft above visible Gboard. The coordinator documented actual durable Latin input capture and physical calendar pointer navigation. Neither screenshot proves spoken accessibility or a Korean keyboard subtype.

No merge/deployment approval is given. The delivered debug APK and automated result are reviewable; the repository's strict full gate remains FAIL until the explicitly missing manual evidence is supplied.
