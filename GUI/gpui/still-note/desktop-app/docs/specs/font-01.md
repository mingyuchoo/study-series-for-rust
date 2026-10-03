# FONT-01: Pretendard throughout UI

- Version: 1
- Status: READY
- Spec owner: /root
- Base revision: 3f71b54947b3c7ccc6fa3ab74749b81c738be062
- Request: UI/UX 폰트를 전체적으로 Pretendard로 변경.

## 목표와 범위
현재 저장소는 GPUI native journal이다. 전체 앱 내부 텍스트(heading/navigation/button/body/input/placeholder/stats/help/error)를 Pretendard로 변경한다. OS titlebar 및 emoji/symbol 중 Pretendard 미지원 글리프는 OS fallback을 허용한다. 새 웹 앱은 만들지 않는다. 사용자 요청은 DESIGN.md의 Inter/JetBrains 폰트 요구보다 우선하며 색상/크기/weight/spacing 및 journal 기능은 유지한다.

## 계약과 설계 제약
공식 Pretendard upstream의 full Korean 지원 폰트를 라이선스와 함께 앱에 bundle/register한다. 로컬 font 설치 및 실행 중 네트워크에 의존하지 않는다. 모든 앱 font_family 경로 및 별도 font override를 조사한다. 기본 weight 400, controls600, nav500, heading/stat700이 지원돼야 한다. 이전 bundled fonts는 사용처가 없어지면 제거하고 문서에서 현재 정책을 설명한다. font 이름/registration 오류를 숨기지 않는다. DESIGN.md 원문 및 과거 spec은 보존한다.

## Acceptance criteria

| AC-ID | Given/When | Then | 검증 |
|---|---|---|---|
| AC-01 | 새 앱 context 초기화/UI render | 실제 Pretendard binary 등록, family token/모든 UI override Pretendard; 한국어/영문 및 400/500/600/700 지원 | unit exact family + asset/font metadata registration evidence + independent source audit |
| AC-02 | 폰트 미설치/offline 환경 | embedded font 사용, official source/version/license 기록, 불필요한 Inter/JetBrains asset/registration 제거 | asset/license audit + production constructor render test |
| AC-03 | 600/767/768/1024/1360/1600px 및 focus/menu/action | 기존 bounds/reachability/focus/menu/40px controls contract 유지 | 기존 production E2E 전체 |
| AC-04 | 노트 작성/검색/저장 및 한글 IME | 기존 동작 Unicode/clipboard/IME 유지; 변경에 맞는 문서/테스트 expectation, 기능 assertion 유지 | unit/integration/E2E 전체 + README review |

## 작업/검증 계획
Builder /root/code owns src/** assets/fonts/** README.md. 필요 설정 변경은 root에 먼저 통보. Test /root/test owns tests/** .artifacts/font-01/test*. Reviewer /root/reviewer owns .artifacts/font-01/review*. Root owns this spec and final gate. DESIGN.md 및 이전 spec read-only. 모든 writer 종료 후 builder만 Git Bash scripts/format.sh --write, 지정 파일 checkpoint commit. 독립 verifier는 clean fixed revision에서 C:/Program Files/Git/bin/bash.exe scripts/verify.sh를 순차 직접 실행. format/lint/unit/integration/e2e 5단계 exit0, 각 테스트 >0 및 skip/실패 없음 확인. Reviewer가 동일 SHA diff와 AC/log 원본 독립 확인.

## 질문/가정/위험
필수 질문 없음. 사용자 표현 웹 UI는 이 저장소 native 앱 UI를 지칭한다고 해석한다. Native screenshot API 비활성화로 pixel QA는 불가; production headless bounds와 실제 font metadata/등록 증거로 확인하고 제한 기록한다. OS fallback glyph와 기존 음수 grapheme margin tracking의 native rasterization 차이는 유지 위험으로 보고한다.

## 완료 조건
AC-01..04 전부 증거, 독립 검증/리뷰 동일 clean SHA PASS, 작성자 분리. 실패 최대3회 수정 후 blocker 보고. PASS는 merge/deploy 승인 아님.
