# LAYOUT-01: Complete text and width/height responsiveness

- Version: 1
- Status: READY
- Spec owner: /root
- Base revision: 6a16730968fa451c9c4aa6f2ebc3a39b27044156
- Request: attached screenshot clipped text; complete text and responsive window width/height.

## 목표와 범위
GPUI native journal의 logo가 글자 단위 세로 배치되어 상단 아래로 잘리는 screenshot 문제와 전반적인 긴 텍스트/작은 창 높이 문제를 해결한다. Pretendard/black-yellow와 기능 보존. 필요한 최소 native layout 수정, 새 dependency/임의 텍스트 삭제/ellipsis로 요구 숨김 금지. 음수 grapheme margin 방식이 원인이면 native whole-string shaping 우선; 현재 사용자 clipping fix가 과거 tracking보다 우선한다. 한 줄 입력은 caret-follow horizontal scroll 등 기본 편집 UX로 모든 입력에 접근 가능해야 하며 저장된 긴 노트/컬렉션/오류 문구는 줄바꿈/스크롤로 전체를 읽을 수 있어야 한다.

## Acceptance criteria

| AC-ID | Given/When | Then | Verification |
|---|---|---|---|
| AC-01 | screenshot와 같은 desktop + narrow nav | stillnote wordmark 전체 한 줄로 nav64 안에 배치; 글자 단위 flex wrapping/clipping 없음; heading/stats 한글/영문 정상 shaping | actual native metrics/production render evidence where possible + bounds + source review |
| AC-02 | 긴 한국어/영어 노트, 컬렉션 이름, 오류/설명 및 label | 저장된 내용 삭제/truncate 없이 줄바꿈 또는 스크롤로 끝까지 읽을 수 있음; action buttons/inputs 잘림 없음; 긴 단일 토큰도 viewport horizontal overflow 안됨 | meaningful long-text production E2E + native shaping evidence/source audit |
| AC-03 | width600/767/768/1024/1360/1600 and height400/500/650/900 at representative combinations | width AND height 반응형: nav 유지; main/collection/actions accessible by scroll; compact heights collapse/reflow decorative panels/footer as needed; no inaccessible fixed footer/form; minimum native height adjusted if necessary (600x400 supported) | production E2E actual scrolling/access checks; native min-size/source audit |
| AC-04 | resize + menu + edit/migrate/search/IME/save | 기존 journal 기능, font registration and Unicode/input contracts 유지; responsive resizing doesn't reset state; actions reachable including long entries | all existing unit/integration/e2e + targeted resize/actions |

## Owners / commands
Builder /root/code owns src/** README.md, no other product/config/assets writes unless root transfer. Test /root/test owns tests/** .artifacts/layout-01/test*. Reviewer /root/reviewer owns .artifacts/layout-01/review*. Root owns this spec/final gate. All agents read AGENTS and role. Minimal ponytail skill applies (C:/Users/mingy/.codex/plugins/cache/ponytail/ponytail/4.10.0/skills/ponytail/SKILL.md). Test writer done before builder sole installed Git Bash scripts/format.sh --write + explicit checkpoint (include spec/tests). Independent clean fixed SHA scripts/verify.sh five stages exit0, suite counts>0, no skips/weakening. Reviewer same SHA diff/AC/raw evidence.

## Environment / risk
Headless GPUI uses NoopTextSystem synthetic metrics; cannot alone claim actual native clipping fix. Seek real native metrics/render evidence without altering personal journal using temporary fixture, available screen/render tools where allowed. Native screenshot unavailable if API remains disabled, record limitation; source fix must remove root cause and native fonts still validated. No mandatory user question. Window sizes stated logical pixels. Max3reworks; no weakened expectations to pass. Final gate clean sameSHA AC/tests/reviewerPASS independent owners.
