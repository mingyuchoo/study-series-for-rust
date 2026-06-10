현재 코드베이스는  주소록으로 개발된 프로젝트이다. 현재 프로젝트를 Rob Moore의 “큰 그림에서 세부 실행으로 내려가는 모델”인 약자 VVKIK 로된 개인 목표와 성과 관리 앱으로 만들고자 한다.
* Value (가치)
* Vision (비전)
* Key Result Area (핵심 결과 영역)
* Income Generating Task (소득 창출 업무)
* Key Performance Indicator (핵심 성과 지표)
을 관리하는 앱으로 만들고자 한다. 

vvkik의 구조는 아래와 같다.
```
VVKIK
├─ Value 가치
│  └─ 무엇을 중요하게 여길 것인가?
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
│  └─ KRA 안에서 실제로 돈/성과를 만드는 고가치 실행 업무
│     └─ 가장 우선적으로 시간과 에너지를 써야 함
│
└─ Key Performance Indicator, KPI 핵심 성과 지표
   └─ KRA와 IGT가 제대로 작동하는지 측정하는 피드백 지표
      └─ 결과를 보고 KRA/IGT를 조정하게 함
```
또한 5가지 사항에 대한 관계는 아래와 같다.
```
Value + Vision
    ↓ 방향과 기준
Key Result Areas
    ↓ 집중해야 할 영역
Income Generating Tasks
    ↓ 실제 수익/성과를 만드는 행동
Key Performance Indicators
    ↑ 측정과 피드백으로 전체 시스템 조정
```
위의 기능을 만들기 위한 현재 코드베이스를 정리해서 위의 기능을 구현할 수 있는 스케폴딩 형태로 앱 구조를 만들고자 한다. 너의 생각은 어때?
