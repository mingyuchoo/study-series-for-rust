# RELEASE-INSTALLER-CONFIRMATION: 설치파일 생성 확인

- Version: 1
- Status: READY
- Spec owner: /root
- Base revision: ddc23b582f7975bbbcea72ad137293a1551f8c93
- Request: release.ps1을 분석하고 실행하면 설치파일이 만들어지도록 한다.

## 목표와 범위

기존 release.ps1은 format/lint/test/build 후 Inno Setup으로 설치 EXE를 생성하고 존재 여부를 검사한다. 이 흐름을 재사용하며 성공 시 설치파일의 절대 경로를 명시한다. 실제 실행으로 새 EXE, ZIP, 체크섬 생성을 확인한다. 설치 실행, 배포, 서명, 자동 의존성 설치는 범위 밖이다.

## Acceptance criteria

| AC-ID | Given / When | Then | 검증 |
|---|---|---|---|
| AC-01 | 지원 Windows host에서 release.ps1 실행 | 기존 전체 파이프라인을 거쳐 고유 출력 폴더에 새 비어 있지 않은 setup.exe, ZIP, SHA256SUMS.txt 생성; 두 파일의 해시 일치 | 실제 release.ps1 실행 로그, 크기/PE/ZIP/hash 검사 |
| AC-02 | 성공 완료 | Installer: 뒤에 생성된 설치파일의 절대 경로 출력; 기존 Release complete 출력 유지 | recording-command 회귀 테스트 및 실제 로그 |
| AC-03 | 빌드/installer 단계 실패 또는 출력 누락 | 비정상 종료, 성공 메시지 없음; 기존 종료코드/순서/help/target 동작 유지 | 기존 release-scripts 테스트 |
| AC-04 | 고정 clean revision | verify.sh 5단계 PASS, unit/integration/e2e 각각 실제 테스트 1개 이상, 동일 SHA 독립 리뷰 PASS | summary.tsv, 원본 로그, 독립 보고서 |

## 소유권과 검증

- Builder /root/builder: scripts/release.ps1만 수정. 모든 writer 종료 후 전체 format, spec/제품/테스트를 scoped checkpoint commit.
- Test /root/verifier: tests/release-scripts/test_release_scripts.py만 수정; .artifacts/release-installer-confirmation/ 검증 보고서. spec을 먼저 읽고 기대값 작성.
- Reviewer /root/reviewer: .artifacts/release-installer-confirmation/review.md만 수정.
- Orchestrator /root: 이 spec 및 gate 보고서.
- 필수 질문 없음. 도구는 현재 설치된 PowerShell, Rust/MSVC, Inno Setup 사용. Bash는 C:/Program Files/Git/bin/bash.exe 사용.
- .agents/verification.env의 실제 5단계 명령 유지. Builder가 bash scripts/format.sh --write 후 checkpoint; Verifier가 bash scripts/verify.sh, python tests/release-scripts/test_release_scripts.py, pwsh -NoProfile -File scripts/release.ps1 순차 실행.
- 수정 중 검증 금지. output/target 캐시 공유로 모든 실제 명령 순차 실행. 생성물은 ignored .artifacts/에 보관.
- 실패를 숨기거나 assertion을 약화하지 않는다. 미충족 AC/도구/리뷰는 최종 FAIL이다.
