현재 코드베이스는  주소록으로 개발된 프로젝트이다. 현재 프로젝트를 Rob Moore의 "큰 그림에서 세부 실행으로 내려가는 모델"을 기반으로 한 약자 IKIK 로된 개인 목표와 성과 관리 앱으로 만들고자 한다.
* Identity (정체성)
* Key Result Area (핵심 결과 영역)
* Income Generating Task (소득 창출 업무)
* Key Performance Indicator (핵심 성과 지표)
을 관리하는 앱으로 만들고자 한다. 

ikik의 구조는 아래와 같다.
```
IKIK
├─ Identity 정체성
│  └─ 나는 어떤 사람이 될 것인가?
│     └─ Key Result Area 핵심 결과 영역의 기준/필터가 됨
│
├─ Key Result Area, KRA 핵심 결과 영역
│  └─ 정체성을 증명하기 위해 반드시 집중해야 하는 3~7개 핵심 영역
│     └─ Income Generating Task 소득 창출 업무를 선별함
│
├─ Income Generating Task, IGT 소득 창출 업무
│  └─ Key Result Area 안에서 실제로 돈/성과를 만드는 고가치 실행 업무
│     └─ 가장 우선적으로 시간과 에너지를 써야 함
│
└─ Key Performance Indicator, KPI 핵심 성과 지표
   └─ Key Result Area와 Income Generating Task가 제대로 작동하는지 측정하는 피드백 지표
      └─ 결과를 보고 Key Result Area/Income Generating Task를 조정하게 함
```
또한 4가지 사항에 대한 관계는 아래와 같다.
```
Identity
    ↓ 방향과 기준
Key Result Areas
    ↓ 집중해야 할 영역
Income Generating Tasks
    ↓ 실제 수익/성과를 만드는 행동
Key Performance Indicators
    ↑ 측정과 피드백으로 전체 시스템 조정
```
위의 기능을 만들기 위한 현재 코드베이스를 정리해서 위의 기능을 구현할 수 있는 스케폴딩 형태로 앱 구조를 만들고자 한다. 너의 생각은 어때?

---

## 변경 이력

- 2026-06-11: 제임스 클리어의 Atomic Habits 정체성 기반 변화 개념을 반영해 최상위 단계를 Value(가치, "무엇을 중요하게 여길 것인가?")에서 Identity(정체성, "나는 어떤 사람이 될 것인가?")로 교체했다. 앱 이름도 VVKIK에서 IVKIK로 변경했다. 클리어의 모델에서 가치관은 정체성의 부분집합이므로 개념의 손실 없이 행동 유발력이 더 강한 프레이밍으로 확장한 것이다.
- 2026-06-12: Vision 단계를 제거하고 계층을 Identity → Key Result Area → Income Generating Task → Key Performance Indicator 4단계로 줄였다. 앱 이름도 IVKIK에서 IKIK로 변경했다. 클리어의 모델에서 정체성("나는 ~한 사람이다")이 곧 방향을 함의하므로 Vision은 Identity의 부연이거나 Key Result Area의 묶음 라벨에 그치는 중간층이었다.
- 2026-06-12: 내부 식별자도 IKIK로 통일했다(IkikItem 타입, ikik_items 테이블, ikik.sqlite 파일명, 번들 identifier com.mingyuchoo.ikik, 디렉터리명 ikik). 기존 앱 데이터는 모두 삭제하고 새로 시작하기로 했으므로 구버전 데이터베이스 마이그레이션(vision 병합, aggregation 컬럼 추가)은 제거했다.
- 2026-06-12: 마감 기한(due_date)을 도입했다. Rob Moore의 Leverage(파킨슨 법칙: 일은 주어진 시간만큼 늘어진다)와 Clear의 실행 의도 개념에 따라 마감은 실행 단위인 Income Generating Task에 두고, Key Result Area에도 선택적으로 허용한다. Key Performance Indicator의 날짜는 지표가 끝나는 날이 아니라 목표값을 달성해야 하는 시점이므로 "목표 달성일"로 라벨과 표기("~까지")를 분리했다. Identity는 마감 없이 지속되는 단계라 검증에서 거부한다. 칩 표기는 날짜+D-day 병기(예: "6월 15일 · D-3")로, 7일 이내 임박은 검정 반전, 지난 마감은 오류 톤을 쓴다. 마감 변경은 기존 변경 이력(item_revisions)에 due_date 필드로 기록되고, 구버전 데이터베이스에는 ALTER TABLE로 컬럼을 멱등하게 추가한다.
- 2026-06-12: Key Performance Indicator 상세의 실적 기록 입력을 숫자 직접 입력에서 − / 숫자(직접 입력 불가) / + 스테퍼로 바꿨다. 길게 누르면 자동 반복(450ms 후 90ms 간격)되고, 한 클릭의 변화량은 지표가 스스로 정하는 스텝 칩(±0.1/±1/±10/±100 중 2~3개)으로 바꾼다 — 합계형이 아닌 작은 지표는 ±0.1을, 목표값이 30 이상이면 ±10을, 300 이상이면 ±100을 더하며, 칩이 ±1 하나뿐이면 칩 줄을 숨긴다. 시작값은 합계형(증분 기록)은 0, 최신값·평균형은 직전 기록값이고 음수 측정값은 만들 수 없다.
- 2026-06-12: 한국어 화면의 단계 명칭을 우리말로 바꿨다 — Identity→정체성, Key Result Area→핵심 결과 영역, Income Generating Task→소득 창출 업무, Key Performance Indicator→핵심 성과 지표. 표기는 프런트엔드의 kind_label(kind, lang) 한 곳이 결정하고(영어 화면은 contracts의 영문 풀네임 유지), 탭·트리 행 태그·브레드크럼·폼 세그먼트·대시보드 카드·변경 이력 값·헤더 문구와 검색 placeholder까지 모두 이 함수를 거친다. 와이어 포맷(identity/kra/igt/kpi)과 데이터는 그대로다.
- 2026-06-12: 단계 탭과 대시보드 카드의 수량 표기 중복을 정리했다. 탭은 숫자 배지를 없애 이동 수단으로만 남기고, 대시보드 카드가 상태 분포를 전담한다 — 상태별 한 줄(라벨 · 비율 막대 · 수량) 세 줄로, 0인 상태도 빈 트랙으로 자리를 지켜 분포가 한눈에 비교된다. 막대 농도는 "지금 살아 움직이는 것"이 진하도록 진행 중(잉크) > 일시 중지(중간 회색) > 완료(옅은 회색)다. 상태 라벨도 한국어 화면에서 진행 중/일시 중지/완료로 현지화했다(status_label이 Lang을 받음).
- 2026-06-12: 응집·결합 리팩토링 4건. ① 단계·상태·집계 enum의 contracts/domain 중복을 없앴다 — contracts를 공유 커널로 인정하고 domain이 contracts에 의존해 재사용하며(taxonomy.rs 삭제), 수동 변환 함수 6개도 함께 사라졌다. "domain은 내부 크레이트에 의존하지 않는다" 원칙은 "contracts(공유 커널) 의존만 허용"으로 갱신. ② 측정값 스테퍼를 MeasurementStepper 컴포넌트로, 기록+토스트+실행취소 흐름을 use_quick_record 훅으로 추출 — 상세 패널과 단계 탭 퀵 기록 팝오버가 같은 입력·기록 경로를 쓰고, 팝오버도 스테퍼를 쓰게 됐다. ③ 백엔드 AppState를 유스케이스 10개 나열에서 Arc<dyn IkikRepository> 하나로 축소(유스케이스는 무상태라 커맨드가 즉석 생성). ④ i18n.rs(500줄)를 화면 영역별 서브모듈(common/dashboard/form/detail/records/tree)로 분할. 표기 원칙: i18n은 화면 문구, models는 도메인 값(단계·상태·집계)의 레이블.
- 2026-06-12: 후속 리팩토링 7건. ① 브레드크럼을 Breadcrumb 컴포넌트로 추출(상세·수정 폼 공유). ② 데이터 접근 경로를 스토어 하나로 통일 — 화면-국소 데이터(측정 기록·변경 이력·잔디)도 store.load_*를 거치고, 컴포넌트는 IkikService를 직접 부르지 않는다. ③ 빠른 추가의 생성 요청 조립을 QuickAddData::to_create_request로 이동. ④ 트리 드래그앤드롭 상태 머신을 TreeDrag 훅으로 추출. ⑤ 폼의 생성 모드 구조 선택(단계 세그먼트+상위 항목)을 StructureFields 하위 컴포넌트로 분리. ⑥ UpdateItemUseCase를 검증→상위 확정→적용→이력→재집계 단계 함수로 분해. ⑦ 백엔드 검증 오류를 구조화 — contracts에 ValidationIssue(코드+파라미터)를 정의하고 DomainError::InvalidIkikData(String)를 Validation(ValidationIssue)로 교체, 와이어 ApiError에 issue 필드를 실어 프런트엔드가 현재 언어(한국어 단계명 포함)로 문구를 조립한다. NotFound도 현지화. 백엔드 message는 한국어 기본 문구로 유지(로그·하위 호환).
- 2026-06-12: 사용/관리 2모드를 도입했다(A안: 완전 숨김형). 사용 모드(기본값)는 조회·검색·실적 기록만 남기고 구조 변경 진입점 — 상세의 수정·삭제, 헤더의 새 항목, 트리의 삭제·하위 추가·정체성 추가·드래그 이동 — 을 화면에서 숨겨 실수로 기준 문서를 바꾸는 것을 막는다. 측정 기록은 실행 취소가 있고 일상 사용의 본질이라 항상 허용. 전환은 헤더의 자물쇠 토글(잠김=사용, 열림+검정 반전=관리)이고 선택은 테마·언어처럼 localStorage("mode")에 보존된다.
