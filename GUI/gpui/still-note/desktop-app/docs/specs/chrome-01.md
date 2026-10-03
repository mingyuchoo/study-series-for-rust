# CHROME-01: Merge navigation and Windows titlebar

- Version: 1
- Status: READY
- Spec owner: /root
- Request: 기존 상단 탐색 바와 Windows 타이틀바를 GPUI로 합침.

## Contract
User authorizes custom GPUI titlebar integrated into existing64px nav; native system titlebar hidden using GPUI0.2.2 supported options. Left logo/navigation, right minimize/maximize-restore/close; remaining blank title area drags window. Existing mobile hamburger retained, controls remain visible at600x400. Use existing GPUI platform hit-test/control APIs rather than dependency/manual Win32 replacement. Double-click title blank maximizes/restores, native borders/resize/system close and snap supported as backend allows. Button/menu/input areas must not startdrag or title-doubleclick. Show correct maximize/restore indication. Preserve full-width24gutter, Pretendard/colors/journal/draft/IME/storage. No personalapp shutdown or journal writes.

| AC | Expected | Evidence |
|---|---|---|
| AC-01 | WindowOptions hides systemtitlebar Windows; integratednav64 with fullwidth and controls/rightaligned; no duplicate chrome | source/native hidden window creation/render + bounds at600/768/1360/1920/2880 and short400 |
| AC-02 | blank title area windowmove/doubleclick; minimize/maximize/restore/close platform actions, maximize state reflected; interactive nav neverdrag | platformcontrol API source/hit-test routing review + native test observations where possible; explicit limitations for physical OS interactions |
| AC-03 | compact menu/control targets stay visible no overlap; resize wide→600x400 preserves state/access; oldfont/labels/layoutcontracts remain | production E2E bounds/menu/draft/action/scroll and native font test |
| AC-04 | all journal domain/store/input workflows unchanged | completeunit/integration/e2e |

## Ownership / gate
Builder/root/code src/main.rs src/ui.rs src/theme.rs README.md only, configuration changes requestroot. Test/root/test tests/** artifacts/chrome-01/test*. Reviewer/root/reviewer artifacts/chrome-01/review*. Root owns spec/final. ReadAGENTS/role, ponytailminimalnativeexistingAPI, graftcallersbeforechanges. Coordinate controls/selectors; allwritersDONE before builderalone GitBashformatwrite+explicitcheckpointincludespec. Independent cleanfixedSHA fullGitBashverify5stageexit0 counts>0 no skips/weakening, sameSHA ACreviewPASS. Isolate CARGO_TARGET_DIR artifacts/button-01/target ifdefaultexe locked. No merge/deploy. Max3rework.

## Validation limits
GPUIheadless usesNoop metrics/platform; cannot establish realdrag/Snap/AltF4 system behavior. Seek native hiddenwindow structural/backend observations without visible personalwindow controls. Native physical pointer/screenshot APIs currentlydisabled; record untestedmanual interaction explicitly, never claim observed Snap if only source supports. Avoid test close quit killing runner; use isolatednative app/temporaryjournal.
