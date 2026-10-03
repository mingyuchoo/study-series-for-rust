# BUTTON-01: Contain fixed action labels

- Version:1
- Status:READY
- Spec owner:/root
- Base revision:e65c0c99a8c05adde7d8f0ec037b47f1455ec757
- Request: screenshot lower-left create-collection text wraps vertically outside button.

## Contract
Fix shared button label sizing at root, smallest native layout change (ponytail skill already read). Fixed action labels including create-collection must render complete, centered, single-line inside40px buttons; do not clip/ellipsis/delete text to hide bug. Dynamic collection/index/trace long labels retain normal wrapping/natural height and fulltext. Preserve Pretendard, styling, responsive600x400..1600x900 and actions. Existing fixed button intrinsic width must allow full text plus padding; sidebar width must fit current create label or use supported responsive arrangement without text loss.

| AC-ID | Given/When | Then | Evidence |
|---|---|---|---|
| AC-01 | desktop208px sidebar create-collection and compact creation action | entire creation label fits button content bounds single-line, no vertical overflow; fixed sibling action labels same policy | real native Pretendard shape14/600 label width vs available inner width and production label/button bounds; source audit |
| AC-02 | long named collection/index/trace | full long labels still wrap with natural height and contained edges; existing workflows/responsive/scroll preserved | retained targeted long-label E2E and all suites |
| AC-03 | click creation at desktop/narrow/short windows | exact collection name created and selected, focus/style/input/state unaffected | production E2E real click and persisted journal evidence |

## Owners and verification
Builder/root/code owns src/ui.rs only (README only if policy materially changes). Test/root/test owns tests/** .artifacts/button-01/test*. Reviewer/root/reviewer owns .artifacts/button-01/review*. Root spec/final report. Read AGENTS/role; graft callers before button edit. No samefile edits, tests done before builder sole Git Bash format write/checkpoint(include thisspec). Independent cleanSHA full scripts/verify.sh5stages/counts>0 + AC evidence, reviewer sameSHA PASS. No weakening assertions/skips. Native hidden draw/shape test reuse if needed, headless metrics alone cannot prove native font width. No screenshot available; report limitation. Final cleanSHA, independent owners, allACPASS. Max3rework.
