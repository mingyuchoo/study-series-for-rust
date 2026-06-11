현재 코드베이스는  주소록으로 개발된 프로젝트이다. 현재 프로젝트를 Rob Moore의 "큰 그림에서 세부 실행으로 내려가는 모델"을 기반으로 한 약자 IVKIK 로된 개인 목표와 성과 관리 앱으로 만들고자 한다.
* Identity (정체성)
* Vision (비전)
* Key Result Area (핵심 결과 영역)
* Income Generating Task (소득 창출 업무)
* Key Performance Indicator (핵심 성과 지표)
을 관리하는 앱으로 만들고자 한다. 

ivkik의 구조는 아래와 같다.
```
IVKIK
├─ Identity 정체성
│  └─ 나는 어떤 사람이 될 것인가?
│     └─ Vision 비전의 기준/필터가 됨
│
├─ Vision 비전
│  └─ 어디로 갈 것인가?
│     └─ Key Result Area 핵심 결과 영역을 정함
│
├─ Key Result Area, KRA 핵심 결과 영역
│  └─ 비전을 이루기 위해 반드시 집중해야 하는 3~7개 핵심 영역
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
또한 5가지 사항에 대한 관계는 아래와 같다.
```
Identity + Vision
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
