# RELEASE-SCRIPTS: Windows 배포 산출물

- Version: 2
- Status: READY
- Spec owner: /root
- Base revision: 84bb0d8040768dd9ba46749c83354912a71bbae8
- Request: scripts/release.ps1 및 scripts/release.sh로 포맷, 린트, 테스트, release 빌드, 번들, 설치 파일 생성

## 범위 및 인터페이스

Windows GPUI 앱이다. 설치된 Inno Setup 6.3 이상을 사용한다. PowerShell 7.4 이상이 실제 구현이며 Git Bash wrapper는 같은 구현에 인수를 전달한다. 별도 패키지 관리자/framework를 추가하지 않는다. 도움말 --help, 선택 --target x86_64-pc-windows-msvc 또는 aarch64-pc-windows-msvc를 제공한다. 기본 target은 rustc host다. ISCC_PATH 환경변수 또는 PATH/표준 per-user 및 Program Files 설치 경로에서 컴파일러를 찾는다. 지원하지 않는 target, 잘못된 옵션, 누락 도구는 포맷 전에 오류로 종료한다. WSL/Linux/macOS 패키징은 범위 밖이다.

format write → strict lint → Cargo all-targets test(test-support) → doctest → cargo build --release --locked --target TARGET → 번들 ZIP → Inno 설치 EXE → SHA256 checksum 순서로 실행한다. lint 및 테스트는 실행 가능한 rustc host target에서 수행하고 --target은 배포용 release binary의 target을 지정한다. cross build에는 별도 target 및 MSVC 도구가 필요하다. Cargo metadata --locked --no-deps로 앱 이름/버전/실제 target 디렉터리를 얻는다. Cargo dependency 명령에는 --locked를 사용한다. 단계 오류/종료코드를 보존하며 후속 단계/완료 메시지를 출력하지 않는다.

산출물은 앱 루트 .artifacts/releases/의 고유한 실행 디렉터리에 생성하며 이전 산출물 또는 사용자 데이터 삭제/재사용은 하지 않는다. bundle에는 stillnote.exe, README, GPUI 및 Pretendard 라이선스를 포함한다. 폰트는 기존 include_bytes!로 실행 파일에 포함된다. exe 및 zip 이름에 버전/architecture를 명시한다. installer는 per-user 설치, 시작 메뉴 바로가기, 기본 uninstall 지원, architecture 및 Windows 10 이상 제한(MinVersion=10.0)을 갖고 개인 journal 데이터는 제거하지 않는다. 앱을 빌드/설치 검증 시 자동 실행하지 않는다. installer는 서명하지 않으며 문서에 명시한다. 실제 ARM64 PE가 VCRUNTIME140.dll을 import하므로 Microsoft VC++ 2015–2022 matching architecture runtime을 문서화한다.

## Acceptance criteria

| AC-ID | 조건/실행 | 기대 동작 | 검증 |
|---|---|---|---|
| AC-01 | 두 entrypoint 정상 실행 | 전체 단계 순서, release target, locked/test-support, 동일 구현 | 독립 recording-command 테스트 및 실제 pipeline |
| AC-02 | cwd/경로 공백, --target, --help, invalid/missing tools | 인수/cwd 보존; help 무부수효과; 잘못된 입력/필수도구 누락 preflight 거부 | 독립 셸 테스트 |
| AC-03 | 단계별 실패/없어진 binary/installer | 종료 코드 보존, 후속 단계 미실행, stale 산출물을 성공으로 취급하지 않음 | 실패 주입 및 내용 검사 |
| AC-04 | 실제 번들/설치 생성 | ZIP+EXE+checksum 생성, exe 및 라이선스 포함, metadata/target_directory 반영 | 실제 빌드 산출물/ZIP/PE/hash 검사 |
| AC-05 | 실제 installer | per-user install/uninstall, architecture 및 Windows 최소버전 일치, 설치 파일 hash 일치, 개인 데이터 비접근/보존 | 임시 install dir에 silent install/uninstall, template MinVersion 검토 및 Inno compile, 라이선스/바로가기 검토, 앱 미실행 |
| AC-06 | 고정 clean SHA | verify.sh 5단계 PASS; unit/integration/e2e 각1+ 실제실행, 전체 release pipeline PASS | 독립 도구 로그 및 동일 SHA 리뷰 |

## 소유권과 검증

- Builder /root/release_code: scripts/release.ps1, scripts/release.sh, packaging/stillnote.iss, README.md. 필요 공통파일 변경은 사전 owner 이전.
- Test /root/release_test: tests/release-scripts/ 및 tests/powershell/run-tests.ps1 예상 script 목록 갱신; .artifacts/release-scripts/ 증거.
- Reviewer /root/release_review: .artifacts/release-scripts/ 리뷰만.
- Orchestrator: 이 spec 및 최종 보고서.

기존 전체 format은 모든 writer 종료 뒤 Builder 단독 bash scripts/format.sh --write, scoped checkpoint commit. Verifier는 고정 clean SHA에서 verify.sh 및 actual release pipeline을 순차 실행한다. target/ 캐시 재사용은 순차 실행으로만 허용한다. 테스트 의존성 및 인스톨러 실행은 isolated artifact 경로 사용; 설치·제거 검증은 완료 후 정리하고 개인 저널/기존 Stillnote 설치는 접근하지 않는다. 필수 질문 없음. 릴리스 파일 생성은 배포/업로드/서명/개인 설치 실행 승인을 포함하지 않는다.
