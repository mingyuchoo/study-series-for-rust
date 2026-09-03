# Q. 가장 정확도가 높은 AI 에이전트 시스템의 구조를 설계해달라.

# A. 
## 가장 정확도가 높은 AI 에이전트 시스템 구조

목표를 **“차세대 시스템을 가장 빨리 만드는 것”**이 아니라 **“Legacy 기능을 최대한 빠짐없이 보존하면서 신규 시스템의 오류를 지속적으로 제거하는 것”**으로 잡으면, 구조는 단순한 Multi-Agent Coding System이 아니라 **Evidence-Driven Closed-Loop Modernization System**이 되어야 합니다.

AWS Transform도 현재 legacy 분석, business logic 추출, refactoring, functional equivalence testing을 분리된 단계로 다루고 있고, 테스트 계획·데이터 수집·자동화 스크립트·legacy/modernized 결과 비교를 agentic 기능으로 제공하는 방향입니다.  OpenAI와 Anthropic 역시 장시간 코딩에서는 모델 자체만큼 **agent harness 설계**가 성능에 크게 영향을 준다고 보고하고 있습니다. 

제가 설계한다면 아래 구조로 갑니다.

---

### 1. 전체 Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                MODERNIZATION CONTROL PLANE                 │
│                                                             │
│   Policy / Quality Gate / Risk / Evidence / Audit / HITL   │
└────────────────────────────┬────────────────────────────────┘
                             │
                             ↓
                 ┌─────────────────────────┐
                 │   ORCHESTRATOR AGENT    │
                 │ Plan / Route / Retry    │
                 └────────────┬────────────┘
                              │
        ┌─────────────────────┼─────────────────────────┐
        │                     │                         │
        ↓                     ↓                         ↓
┌───────────────┐    ┌────────────────┐       ┌─────────────────┐
│ DISCOVERY     │    │ SPECIFICATION  │       │ BEHAVIOR MINING │
│ AGENTS        │    │ AGENTS         │       │ AGENTS          │
│               │    │                │       │                 │
│ Source        │    │ Business Rule  │       │ Prod Trace      │
│ DB            │    │ Requirements   │       │ Logs            │
│ Interface     │    │ Invariants     │       │ Transactions    │
│ Dependency    │    │ Acceptance     │       │ State Changes   │
└───────┬───────┘    └───────┬────────┘       └────────┬────────┘
        │                     │                          │
        └─────────────────────┼──────────────────────────┘
                              ↓
                ┌─────────────────────────────┐
                │ SYSTEM KNOWLEDGE GRAPH      │
                │                             │
                │ Req → Rule → Code → DB     │
                │ → Interface → Scenario     │
                │ → Test → Evidence          │
                └─────────────┬───────────────┘
                              │
                              ↓
        ┌────────────────────────────────────────────┐
        │        MODERNIZATION BUILDER PLANE         │
        │                                            │
        │ Architecture Agent                         │
        │ Decomposition Agent                        │
        │ Coding Agents                              │
        │ Data Migration Agent                       │
        │ Interface Migration Agent                  │
        └────────────────────┬───────────────────────┘
                             │
                             ↓
                       NEXT SYSTEM
                             │
                             ↓
┌──────────────────────────────────────────────────────────────┐
│                VERIFICATION FACTORY                         │
│                                                              │
│ Golden Master │ Replay │ Differential │ State │ Interface   │
│ Boundary │ Property │ Mutation │ Fault │ Concurrency         │
└─────────────────────────┬────────────────────────────────────┘
                          │
             ┌────────────┴────────────┐
             ↓                         ↓
           PASS                       FAIL
             │                         │
             │                         ↓
             │                ┌──────────────────┐
             │                │ RCA AGENT        │
             │                └────────┬─────────┘
             │                         ↓
             │                ┌──────────────────┐
             │                │ FIX AGENT        │
             │                └────────┬─────────┘
             │                         │
             │                         └──────┐
             │                                │
             ↓                                │
       QUALITY GATE ←──────────────────────────┘
             │
             ↓
      FE-99.9 / Critical-100
```

핵심은 **Builder와 Verifier를 구조적으로 분리하는 것**입니다.

---

## 2. 핵심 원칙 1: AI가 Specification을 만들고 AI가 코드를 만들되, AI가 최종 정답을 결정하지 않는다

가장 위험한 구조는 이것입니다.

```text
Claude
  ↓
요구사항 분석
  ↓
코드 작성
  ↓
테스트 작성
  ↓
"제가 만든 코드가 맞습니다."
```

이 구조에서는 같은 reasoning bias가 계속 이어집니다.

대신:

```text
Discovery Agent
     ↓
Specification Agent
     ↓
Builder Agent
     ↓
Independent Test Agent
     ↓
Deterministic Verification
     ↓
Adversarial Agent
```

로 분리해야 합니다.

그리고 최종 PASS/FAIL은 가능한 한 **프로그램으로 판정**해야 합니다.

---

## 3. Agent #1 — Discovery Agent

이 Agent의 목표는 코딩이 아닙니다.

**Legacy에서 무엇이 존재하는지 최대한 빠짐없이 발견하는 것**입니다.

입력:

```text
Source Code
DB Schema
Stored Procedures
Config
JCL / Batch
API
MQ
File Interface
Screen
Reports
IAM/Authorization
Git History
Tickets
Documents
Logs
```

출력:

```text
Application Map
Dependency Graph
Call Graph
Data Lineage
Interface Graph
Batch Graph
Domain Classification
Potential Business Rules
Dead / Suspicious Code
```

이 단계에서는 모델 하나만 사용하지 않는 편이 좋습니다.

예:

```text
GPT-5.6 Sol
→ 전체 dependency / architecture / large-scale decomposition

Claude
→ 긴 소스와 업무 문맥 분석

Static Analyzer
→ AST / Call Graph

DB Analyzer
→ Schema / SQL / Lineage
```

즉 **AI + deterministic tooling**을 결합합니다.

AWS Transform 역시 시스템 수준 컨텍스트와 traceability가 부족한 일반-purpose AI 접근을 enterprise modernization의 약점으로 지적하고 있습니다. 

---

## 4. Agent #2 — Business Rule Mining Agent

이 Agent가 매우 중요합니다.

Source code를 다음 형태로 바꿉니다.

```text
BR-LOAN-000183

Name:
VIP 고객 중도상환 수수료 면제

Condition:
customer.grade = VIP
AND loan.age > 3Y
AND repayment.type = EARLY

Result:
fee = 0

Source:
LOAN231.cbl:2912-2947

DB:
CUSTOMER
LOAN_MASTER

Related Interfaces:
IF-238

Observed Production Cases:
18,221

Confidence:
0.97
```

여기서 중요한 필드가:

#### Confidence

입니다.

모든 업무규칙을 동일한 확실성으로 취급하면 안 됩니다.

예:

```text
0.99   코드 + 문서 + Production 일치
0.95   코드 + Production 일치
0.85   코드에서만 발견
0.65   추론된 규칙
0.40   의미 불명
```

이 **Rule Confidence Score**가 나중에 Uncertainty를 계산하는 핵심 데이터가 됩니다.

---

## 5. Agent #3 — Behavior Mining Agent

제가 이 Agent에 가장 큰 투자를 합니다.

왜냐하면 코드보다 **실제 Production Behavior가 더 강한 증거**이기 때문입니다.

수집:

```text
Production Requests
Responses

DB Before / After

External API Calls
MQ Events

Batch Input / Output

Reports

Error / Retry

Lock / Transaction State

Execution Trace

Timing / Ordering
```

그리고 다음 형태의 **Behavior Record**를 만듭니다.

```text
Behavior ID: BH-92182012

Initial State:
...

Input:
...

Legacy Output:
...

DB State Change:
...

Events:
...

External Calls:
...

Timing:
...

Related Business Rules:
BR-183
BR-283
BR-2819
```

이것이 나중에 Golden Master가 됩니다.

---

## 6. 모든 정보를 System Knowledge Graph로 통합

이것이 Agent system의 사실상 장기 기억입니다.

```text
Requirement
   ↓
Business Function
   ↓
Business Rule
   ↓
Source Code
   ↓
Database
   ↓
Interface
   ↓
Production Behavior
   ↓
Test Scenario
   ↓
Verification Evidence
```

예:

```text
REQ-1203
   │
   ├── BR-2839
   │      ├── LOAN221.cbl
   │      ├── CUSTOMER.grade
   │      ├── IF-PAYMENT-20
   │      └── TEST-88291
   │
   └── BR-2840
          ├── ...
```

이 graph가 없으면 Agent가 프로젝트가 길어질수록 context를 잃습니다.

반대로 이것이 있으면 Agent가 매번 전체 수백만 줄 코드를 다시 읽을 필요가 없습니다.

---

## 7. Agent #4 — Architecture Agent

이 Agent는 코드를 작성하지 않습니다.

목표는:

> Legacy의 기능을 어떤 Next Architecture로 mapping할지 결정

입니다.

예:

```text
Legacy Module A
Legacy Module B
Legacy Module C

       ↓ analysis

Customer Domain
Loan Domain
Settlement Domain
Accounting Domain
```

그리고:

```text
BR-1029 → Loan Service
BR-2818 → Accounting Service
...
```

처럼 rule ownership을 결정합니다.

#### 중요한 원칙

Architecture Agent의 결정에는 반드시:

```text
Decision
Evidence
Alternatives
Risks
Affected Rules
Affected Tests
```

가 붙어야 합니다.

---

## 8. Agent #5 — Builder Agents

여기에서 Codex/GPT-5.6 Sol 같은 코딩 Agent를 적극적으로 활용합니다.

한 개 Agent에게 애플리케이션 전체를 주지 않습니다.

예:

```text
Agent Pool

Domain Agent
API Agent
Data Agent
Batch Agent
UI Agent
Integration Agent
IaC Agent
```

각 Agent에 **좁은 bounded context**를 줍니다.

예를 들어 Loan Agent에는:

```text
Loan requirements
Loan business rules
Loan source code
Loan DB schema
Loan interfaces
Loan golden tests
Architecture constraints
```

만 제공합니다.

이것이 context pollution을 줄입니다.

OpenAI도 GPT-5.6에서 반복적 agent 작업의 효율을 위해 context bloat를 줄이고 harness를 설계하는 접근을 설명하고 있습니다. 

---

## 9. 가장 중요한 부분 — Verification Factory

Builder보다 여기에 더 많은 기술 투자를 해야 합니다.

최소 **8개의 독립 검증 Agent/Engine**을 둡니다.

| 검증 Agent | 역할 |
|---|---|
| Golden Replay | 실제 Legacy behavior 재현 |
| Differential | Legacy ↔ Next 결과 비교 |
| State Verification | DB 상태 전이 비교 |
| Boundary Agent | 경계조건 생성 |
| Property Agent | 업무 불변조건 검증 |
| Mutation Agent | 테스트가 실제 오류를 잡는지 검증 |
| Fault Agent | 장애/Retry/Timeout 검증 |
| Concurrency Agent | 동시성/순서 검증 |

---

## 10. Verification Agent #1 — Golden Replay

가장 높은 신뢰도의 test입니다.

예를 들어 5년간 실제 거래가 10억 건 있다면:

```text
1,000,000,000 Production Cases
          │
          ├───────── Legacy
          │
          └───────── Next
```

를 replay합니다.

단순 response뿐 아니라:

```text
Output
DB State
Events
Ledger
Files
Interfaces
```

를 비교합니다.

---

## 11. Verification Agent #2 — Differential Comparator

모든 field를 byte-by-byte 비교하면 안 됩니다.

그래서 각 field마다 equivalence rule을 정의합니다.

```text
Customer ID       EXACT
Amount            EXACT
Accounting Code   EXACT
Interest          EXACT
Timestamp         ±2 sec
Transaction UUID  FORMAT + UNIQUE
JSON Array        ORDER INSENSITIVE
Internal Trace ID IGNORE
```

그리고 중요한 business output은 **무조건 deterministic comparator**를 사용합니다.

AI가 최종적으로:

> “비슷한 것 같습니다.”

라고 판단해서는 안 됩니다.

---

## 12. Verification Agent #3 — Business Invariant Agent

이 Agent가 굉장히 중요합니다.

기존 시스템과 신규 시스템의 결과가 둘 다 틀렸을 수도 있기 때문입니다.

금융 예:

```text
Debit Total = Credit Total

Loan Principal
= Previous Principal
- Repayment

Available Limit
≤ Approved Limit

Settlement Total
= Sum(Transaction)
```

같은 invariant를 따로 검증합니다.

그래서:

```text
Legacy vs Next = SAME
```

이어도,

```text
Business Invariant = FAIL
```

이면 차단합니다.

이것이 **Legacy bug를 그대로 이식하는 것**을 막아줍니다.

---

## 13. Verification Agent #4 — Boundary Generation Agent

Source와 Business Rule Graph를 읽어서 자동으로 경계값을 생성합니다.

```text
IF age >= 65
AND balance > 100000000
```

라면:

```text
age
64 / 65 / 66

balance
99,999,999
100,000,000
100,000,001
```

그리고 조합:

```text
64 + 99,999,999
64 + 100,000,000
...
66 + 100,000,001
```

을 만듭니다.

---

## 14. Verification Agent #5 — Adversarial Agent

이 Agent의 질문은 하나뿐입니다.

> **“이 시스템을 어떻게 깨뜨릴 수 있는가?”**

Builder Agent와 completely independent context를 줍니다.

예:

```text
Timezone
Leap year
Month-end
Year-end
Duplicate request
Out-of-order event
Negative amount
Max integer
Encoding
Null
Partial failure
Retry
Concurrent update
```

를 적극적으로 공격합니다.

---

## 15. Verification Agent #6 — Mutation Agent

이것은 테스트 시스템 자체를 검증합니다.

예를 들어 Next 코드에서 의도적으로:

```text
>= → >
```

로 바꿉니다.

또는:

```text
365 → 360
```

으로 변경합니다.

그런데 기존 test가 모두 PASS한다면?

**테스트가 나쁜 것입니다.**

Mutation Agent는 이런 가짜 defect를 수천 개 삽입하고:

```text
Injected Defects     10,000
Detected              9,983
Undetected               17
```

을 계산합니다.

#### Mutation Detection Rate

\[
=99.83\%
\]

이 숫자가 매우 중요한 KPI가 됩니다.

---

## 16. Verification Agent #7 — Fault Injection Agent

다음을 의도적으로 발생시킵니다.

```text
DB Timeout
API Timeout
MQ Duplicate
Packet Loss
Node Crash
Partial Commit
Slow Response
Disk Full
Retry
```

그리고 Legacy와 Next가 업무적으로 적절한 recovery를 하는지 비교합니다.

---

## 17. Verification Agent #8 — Concurrency Agent

Enterprise 시스템에서는 필수입니다.

예:

```text
잔액 100만원

Txn A -70만원
Txn B -60만원
```

에서 수백 가지 thread ordering을 생성합니다.

```text
A read
B read
A write
B write

A read
A write
B read
B write

...
```

신시스템의 transaction semantics가 올바른지 확인합니다.

---

## 18. RCA Agent — 실패 원인 분석

Verification이 실패하면 다음 context만 RCA Agent에게 줍니다.

```text
Failed Scenario
Relevant Rule
Legacy Trace
Next Trace
DB Diff
Event Diff
Relevant Source
Recent Changes
Related Tests
```

그러면:

```text
Root Cause:
InterestCalculator.java:298

Legacy:
annualRate / 365

Next:
annualRate / 365.25

Affected Rules:
BR-1202
BR-1203

Affected Scenarios:
82,912

Confidence:
98.2%
```

같은 결과를 냅니다.

---

## 19. Fix Agent는 Builder Agent와 분리

RCA Agent가 직접 수정하게 하지 않는 편이 좋습니다.

```text
RCA Agent
     ↓
Root Cause Hypothesis
     ↓
Fix Agent
     ↓
Patch
     ↓
Review Agent
     ↓
Verification Factory
```

로 갑니다.

즉 한 Agent가:

```text
원인 추정 → 수정 → 검증
```

전체를 자기 확신으로 처리하지 못하게 만듭니다.

---

## 20. 독립 Review Agent를 하나 더 둡니다

여기서는 다른 모델을 쓰는 것이 좋습니다.

예를 들어:

```text
Builder
GPT-5.6 Sol / Codex

Reviewer
Claude

Adversarial
GPT-5.6 Sol 별도 session

Business Review
Claude 별도 session
```

처럼 합니다.

모델 diversification을 쓰는 이유는 완벽한 독립성을 얻기 위해서라기보다 **같은 failure mode의 상관관계를 조금이라도 줄이기 위해서**입니다.

---

## 21. Human Agent도 Architecture의 일부입니다

99.9%를 원하면 사람을 완전히 제거하는 것이 목표가 되어서는 안 됩니다.

사람은 **불확실성이 높은 부분에만 투입**합니다.

예:

```text
Confidence > 0.98
→ 자동 진행

0.90~0.98
→ AI cross review

0.70~0.90
→ Expert review

< 0.70
→ Mandatory SME decision
```

이것을 **Risk-Based HITL**로 운영합니다.

AWS Transform도 주요 modernization 단계에서 human input을 포함하는 HITL 구조를 지원합니다. 

---

## 22. 모든 Agent 위에 Evidence Ledger를 둬야 합니다

이것이 매우 중요합니다.

Agent가:

> “완료했습니다.”

라고 말하는 것은 증거가 아닙니다.

모든 완료 조건을 evidence로 정의합니다.

예:

```text
Function FN-01882

Implemented:
YES

Requirements covered:
100%

Business rules:
38 / 38

Golden tests:
128,281 / 128,281

Boundary tests:
3,918 / 3,918

Mutation score:
99.92%

Production replay:
2,918,221 / 2,918,221

P0 defects:
0

Unexplained differences:
0
```

그래야 Go/No-Go가 객관화됩니다.

---

## 23. 가장 중요한 Quality Gate

저라면 **FE-99.9**라고 단순히 하나의 지표만 쓰지 않습니다.

최종 Gate를 다음처럼 둡니다.

| KPI | Gate |
|---|---:|
| Critical Business Rule Coverage | **100%** |
| P0 Functional Equivalence | **100%** |
| P1 Functional Equivalence | **≥99.999%** |
| 전체 Functional Equivalence | **≥99.9%** |
| Production Behavior Coverage | **≥99.9%** |
| Business Rule Coverage | **≥99.9%** |
| Mutation Detection | **≥99%** |
| Unexplained Difference | **0** |
| P0/P1 unresolved defect | **0** |

특히:

### **Unexplained Difference = 0**

을 절대 기준으로 잡겠습니다.

---

## 24. Accuracy를 더 높이는 핵심은 “Uncertainty Engine”

제가 이 시스템에 추가하고 싶은 가장 중요한 Agent입니다.

각 기능마다:

\[
U = f(R,B,T,D,E,C)
\]

를 계산합니다.

여기서:

- **R** = Requirement confidence
- **B** = Behavior observation coverage
- **T** = Test coverage
- **D** = Dependency uncertainty
- **E** = Equivalence evidence
- **C** = Complexity

예:

```text
FN-10282
Uncertainty: 1.2%
→ Auto

FN-18291
Uncertainty: 7.8%
→ Additional tests

FN-92881
Uncertainty: 24.2%
→ SME mandatory
```

Agent resource를 **확실한 곳에 똑같이 쓰지 않고 불확실성이 높은 곳에 집중**시킵니다.

---

## 25. 실제 Agent Orchestration은 이런 식으로 돌아갑니다

```text
                    ORCHESTRATOR

                         ↓

               Select Business Function

                         ↓

                   Discovery Agent

                         ↓

                  Rule Mining Agent

                         ↓

                Behavior Mining Agent

                         ↓

                Uncertainty Analysis

            ┌────────────┼────────────┐
            ↓            ↓            ↓

           LOW         MEDIUM        HIGH

            │            │            │
            │         Extra AI       SME
            │         Analysis       Review
            └────────────┼────────────┘

                         ↓

                 Architecture Agent

                         ↓

                    Builder Agent

                         ↓

                   Static Verify

                         ↓

                   Unit / Contract

                         ↓

                 Differential Replay

                         ↓

                 Boundary / Property

                         ↓

               Mutation / Adversarial

                         ↓

              Fault / Concurrency Test

                         ↓

                  Verification Gate

                    ↙           ↘

                 PASS           FAIL

                  │              ↓
                  │          RCA Agent
                  │              ↓
                  │          Fix Agent
                  │              ↓
                  │          Review Agent
                  │              │
                  └──────────────┘

                         ↓

                   FE CERTIFIED
```

---

## 26. Claude와 GPT-5.6 Sol의 역할을 나눈다면

2026년 9월 현재라면 저는 **단일 모델 표준화보다 dual-model architecture**를 택하겠습니다.

GPT-5.6 Sol은 현재 OpenAI가 복잡한 전문 업무와 coding, multi-agent workflow를 주요 용도로 제시하고 있으며 Codex에서도 사용할 수 있습니다.  Anthropic 역시 장시간 application development에서 harness 설계가 frontier model의 실제 성능을 크게 끌어올릴 수 있다고 보고합니다. 

추천 역할은:

| 역할 | 우선 모델 |
|---|---|
| Large-scale orchestration | GPT-5.6 Sol |
| Coding / repository modification | GPT-5.6 Sol / Codex |
| Legacy semantic analysis | Claude |
| Business rule extraction | Claude |
| Architecture challenge | Claude + GPT 독립 검토 |
| Test generation | 양쪽 병렬 |
| RCA | GPT/Claude 경쟁 |
| Adversarial verification | Builder와 다른 모델 |
| Final PASS/FAIL | **LLM 아님** |

마지막 줄이 가장 중요합니다.

---

## 27. Accuracy를 극대화하려면 “N-version reasoning”을 일부 적용

모든 것을 세 모델에게 돌리면 비용이 너무 큽니다.

하지만 중요도가 높은 P0/P1에는 적용할 가치가 있습니다.

예:

```text
Business Rule Interpretation

Claude → Answer A
GPT → Answer B
Rule Engine → Evidence C
Production Behavior → Evidence D
```

A와 B가 일치하고 Production까지 일치:

```text
Confidence ↑
```

A와 B가 다름:

```text
Uncertainty ↑
→ 추가 분석
→ SME
```

즉 disagreement를 **위험 신호**로 활용합니다.

---

## 28. 이 구조에서 AI가 90% 정확해도 최종 시스템은 훨씬 높은 수준으로 갈 수 있습니다

핵심 원리가 여기 있습니다.

Builder가 10,000개 변경 중 500개를 틀렸다고 하겠습니다.

```text
Builder Accuracy
95%

Errors
500
```

Verification Harness가 오류의 98%를 찾아냅니다.

```text
Remaining
10
```

수정 후 다시 검증하여 90%를 잡습니다.

```text
Remaining
1
```

그리고 Adversarial/Production shadow에서 잡습니다.

이런 식으로 **iterative error elimination**을 수행하는 것이 목적입니다.

물론 실제 오류는 독립적이지 않기 때문에 단순 확률 곱셈처럼 수렴한다고 보장할 수는 없습니다.

그래서 더 중요한 것이 **독립적 검증 방법의 다양성**입니다.

```text
Code analysis
≠
Replay
≠
Business invariant
≠
Mutation
≠
Production shadow
≠
SME review
```

입니다.

---

## 29. 그래서 정확도 관점에서 가장 중요한 순서는

제 판단으로는 다음입니다.

#### 1위 — Production Behavior Capture

기존 시스템이 실제 무엇을 하는가.

#### 2위 — Functional Equivalence Harness

다른 점을 발견할 수 있는가.

#### 3위 — Business Rule Graph

왜 그렇게 행동하는지 알고 있는가.

#### 4위 — Uncertainty Management

모르는 것을 알고 있는가.

#### 5위 — Independent Verification Agents

Builder와 Checker가 독립적인가.

#### 6위 — Agent Model Capability

Claude인가 GPT인가.

즉 **최고 모델을 사용하는 것보다 위의 1~5가 더 중요합니다.**

---

## 30. 이 시스템을 하나의 제품으로 정의한다면

저라면 SI 회사 내부 명칭을 예를 들어:

### **Autonomous Modernization Assurance Platform — AMAP**

정도로 정의하겠습니다.

구성은:

```text
AMAP

01 Discovery Fabric
02 System Knowledge Graph
03 Business Rule Intelligence
04 Production Behavior Observatory
05 AI Modernization Factory
06 Functional Equivalence Harness
07 Adversarial Verification Factory
08 Uncertainty Engine
09 Evidence Ledger
10 Quality Gate / Certification
```

그리고 AWS Transform, Claude, Codex를 **아래의 교체 가능한 Execution Engine**으로 둡니다.

```text
                    AMAP
                     │
         ┌───────────┼───────────┐
         ↓           ↓           ↓
    AWS Transform   Codex      Claude
         │           │           │
         └───────────┼───────────┘
                     ↓
              Enterprise Systems
```

이렇게 해야 특정 AI 업체에 종속되지 않습니다.

---

### 제가 이 구조에서 가장 중요하게 볼 하나의 KPI

하나만 고르라면 **System Completion %가 아닙니다.**

#### **Residual Uncertainty**

입니다.

예를 들어:

```text
전체 Business Capability             4,821

High-confidence verified             4,793
Known unresolved                        18
Unknown-risk candidates                 10

Residual Uncertainty                 0.58%
```

그리고 Go-Live가 가까워질수록:

```text
5%
→ 2%
→ 0.5%
→ 0.1%
→ 0.01%
```

로 낮추는 것입니다.

**99.9%라는 목표는 결국 “99.9%를 맞혔다”보다 “검증되지 않은 위험을 0.1% 이하로 줄였다”는 방식으로 관리해야 더 정확합니다.**

이 구조가 제가 현재 기술로 설계할 수 있는 **가장 높은 신뢰도의 차세대 전환 AI Agent Architecture**에 가깝습니다. 핵심 경쟁력은 Codex나 Claude 자체가 아니라 **Knowledge Graph + Production Behavior + Verification Harness + Uncertainty Engine + Evidence Ledger**의 결합입니다.

----

# Q. 위 설계를 구현하기 위한 Rust기반의 기술 스택을 추천하라.

# A. 
Rust 기반으로 구현한다면, 저는 **“Rust-only”가 아니라 “Rust-first”**로 갑니다. 핵심 실행부·검증부·고성능 데이터 처리·에이전트 오케스트레이션은 Rust로 만들고, LLM SDK나 일부 분석기는 Python/Java를 필요에 따라 붙이는 구조가 현실적입니다.

### 추천 아키텍처

```text
                    ┌────────────────────────────┐
                    │      Control Plane         │
                    │ Axum / Tonic / Cedar       │
                    └─────────────┬──────────────┘
                                  │
                           Agent Orchestrator
                                  │
                         Temporal / custom DAG
                                  │
          ┌───────────────────────┼───────────────────────┐
          ↓                       ↓                       ↓
 Discovery Workers         Builder Workers       Verification Workers
     Rust                   Rust + LLM               Rust
          │                       │                       │
          └─────────────── Event Bus ────────────────────┘
                             NATS / Redpanda
                                  │
        ┌─────────────────────────┼─────────────────────────┐
        ↓                         ↓                         ↓
 PostgreSQL/Graph            Object Store            Evidence Lake
 Knowledge/Evidence          S3 / MinIO             Parquet / Arrow
        │                                                   │
        └──────────────────── DataFusion ────────────────────┘
```

---

## 1. Rust Core Runtime

기본은 **Tokio + Axum + Tonic + Tower** 조합을 추천합니다.

| 영역 | 추천 |
|---|---|
| Async runtime | **Tokio** |
| REST / HTTP | **Axum 0.8.x** |
| gRPC | **Tonic 0.14.x** |
| Middleware | Tower |
| Serialization | serde / serde_json |
| Error | thiserror / anyhow |
| CLI | clap |
| Config | figment 또는 config |
| Logging | tracing |

2026년 9월 기준 Axum 최신 계열은 0.8.9, Tonic은 0.14.6이며 Tonic은 Tokio/Hyper/Tower 위에서 HTTP/2 gRPC를 제공합니다. 

#### 왜 Axum인가

이 플랫폼은 일반 웹서비스보다 **많은 내부 서비스와 Agent worker**가 생깁니다.

Axum/Tower 조합이면

```text
Auth
Timeout
Retry
Rate limit
Tracing
Policy
Tenant isolation
```

을 계층적으로 관리하기 좋습니다.

외부 Control API는 Axum,

```text
UI / REST Client
      ↓
    Axum
```

내부 고속 RPC는 Tonic으로 분리합니다.

```text
Orchestrator
     ↓ gRPC
Verification Worker
```

---

## 2. Agent Orchestrator

여기가 가장 중요한 선택 중 하나입니다.

저라면 **Temporal을 우선 검토하되, 초기 핵심 workflow abstraction은 Temporal에 종속되지 않게** 설계하겠습니다.

2026년 5월 Temporal은 fully-featured Rust SDK를 Public Preview로 공개했습니다. 아직 Rust SDK가 GA가 아니라는 점은 대형 엔터프라이즈 플랫폼에서는 고려해야 합니다. 

구조는:

```rust
trait AgentTask {
    async fn execute(&self, ctx: AgentContext)
        -> Result<AgentResult>;
}
```

위에

```text
Discover
   ↓
Mine Rules
   ↓
Generate Spec
   ↓
Build
   ↓
Verify
   ↓
RCA
   ↓
Repair
   ↓
Verify
```

라는 DAG를 올립니다.

#### 추천

**초기:** 자체 Rust orchestration abstraction + NATS

**Enterprise 안정화:** Temporal

입니다.

Temporal Rust SDK GA 여부와 실제 production maturity가 확인되면 durable execution을 Temporal에 맡기는 방향이 좋습니다.

---

## 3. Event Bus

저라면 두 가지 중 선택합니다.

#### Option A — NATS JetStream

Agent control message에 적합합니다.

Rust에는 `async-nats`가 있고 2026년 7월 기준 0.50 계열까지 개발되고 있습니다. 

적합한 이벤트:

```text
build.request
build.completed

verification.request
verification.failed

rca.completed
repair.request

human.review.required
```

장점은 단순성과 낮은 latency입니다.

---

#### Option B — Redpanda

Production replay나 수십억 건 behavior stream까지 같은 event backbone에서 처리하려면 Redpanda를 추천합니다.

```text
Production Transactions
Logs
DB CDC
MQ capture
Agent Events
```

까지 처리합니다.

Redpanda는 Kafka protocol 호환성을 제공하고, 2026년 최신 release line은 26.2입니다. 

그래서 저는 규모가 커지면:

```text
NATS
→ Control Plane

Redpanda
→ Data Plane
```

으로 분리하는 편을 더 선호합니다.

---

## 4. Knowledge Graph

여기서는 **Graph DB에 모든 것을 집어넣는 것부터 시작하면 안 됩니다.**

Core system of record는 PostgreSQL로 두는 것을 추천합니다.

```text
PostgreSQL

requirements
business_functions
business_rules
source_units
db_entities
interfaces
test_cases
evidence
relationships
```

그리고 Rust에서는 **SQLx**를 사용합니다.

2026년 7월 SQLx 0.9.0이 나왔으며 PostgreSQL/MySQL/SQLite와 compile-time checked query를 지원합니다. 

#### 제가 권하는 구조

```text
PostgreSQL
    │
Canonical Knowledge Store
    │
    ├── relational queries
    │
    └── graph_projection
             ↓
       Graph Engine
```

graph traversal 자체는 Rust에서 `petgraph`로 처리할 수도 있습니다. 현재 0.8.3 계열입니다. 

초기에는 굳이 Neo4j 같은 별도 DB를 도입하지 않아도 됩니다.

---

## 5. Evidence Lake가 아주 중요합니다

Verification 플랫폼은 시간이 지나면 엄청난 양의 데이터가 생깁니다.

```text
Legacy outputs
Next outputs
DB snapshots
execution traces
test evidence
API recordings
MQ events
diff results
mutation results
```

이걸 PostgreSQL에 넣으면 안 됩니다.

추천:

```text
S3 / MinIO
   +
Parquet
   +
Apache Arrow
   +
DataFusion
```

입니다.

Apache DataFusion은 Rust 기반 query engine이고 2026년 8월 **55.0.0**이 출시되었습니다. distributed engine 확장, MERGE INTO planner, spill backend 같은 기능도 계속 강화되고 있습니다. 

이 플랫폼에는 매우 잘 맞습니다.

예를 들어 수십억 건 replay 결과를:

```sql
SELECT
    business_rule,
    count(*) total,
    sum(pass) passed
FROM replay_result
GROUP BY business_rule;
```

처럼 처리하는 엔진을 Rust 프로세스 내부에 embed할 수 있습니다.

---

## 6. Golden Master / Replay Storage

추천 포맷은:

```text
Metadata
→ PostgreSQL

Large Payload
→ S3

Analytic Evidence
→ Parquet

Hot Cache
→ Redis/Valkey
```

입니다.

예:

```text
TestScenario
 ├── ID
 ├── BusinessRule IDs
 ├── Initial State URI
 ├── Input URI
 ├── Legacy Evidence URI
 ├── Next Evidence URI
 └── Comparator Spec
```

실제 binary payload는 S3에 넣습니다.

---

## 7. Functional Equivalence Engine

이건 반드시 **Rust native**로 만드는 것을 추천합니다.

이 플랫폼의 핵심 IP가 될 수 있기 때문입니다.

대략:

```rust
trait Comparator {
    fn compare(
        &self,
        expected: &Value,
        actual: &Value,
        context: &ComparisonContext,
    ) -> ComparisonResult;
}
```

그리고:

```text
Comparator Registry

ExactComparator
NumericComparator
TimestampComparator
JsonComparator
XmlComparator
DbComparator
EventComparator
AccountingComparator
CustomDomainComparator
```

로 구성합니다.

#### 중요한 설계

Comparator specification도 데이터로 관리합니다.

예:

```yaml
amount:
  comparator: exact

timestamp:
  comparator: tolerance
  tolerance: 2s

transaction_id:
  comparator: format

events:
  comparator: unordered
```

따라서 코드를 수정하지 않고 업무별 비교 정책을 변경할 수 있습니다.

---

## 8. Comparator Plugin은 WebAssembly를 추천

이 부분에서는 **Wasmtime**을 강하게 추천합니다.

Wasmtime은 2026년 8월 기준 48.x까지 올라와 있습니다. 

구조:

```text
Rust Equivalence Engine
         │
         ↓
      Wasmtime
         │
 ┌───────┼─────────┐
 ↓       ↓         ↓
Finance  Telco   Manufacturing
Rules    Rules      Rules
```

장점이 큽니다.

고객별 custom comparator를:

```text
WASM plugin
```

으로 격리하면,

- memory isolation
- execution timeout
- deterministic execution
- language independence

를 얻을 수 있습니다.

이건 SI 플랫폼에서 상당히 좋은 선택입니다.

---

## 9. Business Invariant Engine

단순 Rust 코드로 hardcoding하지 말고 **DSL을 하나 정의**하는 것을 추천합니다.

예:

```text
RULE ACCOUNTING-001

SUM(debit.amount) == SUM(credit.amount)
```

또는:

```text
previous_balance
+ deposits
- withdrawals
== current_balance
```

이 DSL을 Rust로 parser/compiler합니다.

추천 crate:

```text
nom / pest
```

같은 parser 계열을 사용할 수 있습니다.

궁극적으로:

```text
Invariant DSL
     ↓
AST
     ↓
Rust evaluator / WASM
```

형태가 좋습니다.

---

## 10. Policy / Quality Gate

여기에는 **Cedar**를 적극 추천합니다.

Cedar 자체가 Rust 구현이고 2026년 7월 기준 `cedar-policy 4.12.0`이 나와 있습니다. 

원래 authorization policy language지만 다음 같은 Agent governance에도 활용할 수 있습니다.

```text
BuilderAgent
MUST NOT
approve its own changes

CriticalFunction
REQUIRES
IndependentVerifier

Uncertainty > 0.20
REQUIRES
HumanApproval
```

물론 business invariant와 Agent policy를 같은 Cedar policy로 섞지는 않는 것이 좋습니다.

Cedar는:

```text
Who can do what?
```

에 쓰고,

Invariant DSL은:

```text
What must always be true?
```

에 씁니다.

---

## 11. Search / Code Intelligence

Knowledge retrieval은 세 층으로 나누겠습니다.

```text
Exact
Lexical
Semantic
```

#### Exact

PostgreSQL / SQL

#### Lexical search

**Tantivy**

Rust-native search library이고 2026년 현재 0.26.x 계열입니다. 

소스, requirement, ticket, log 등을 indexing합니다.

#### Semantic vector

Qdrant 또는 PostgreSQL + pgvector를 사용할 수 있습니다.

저라면 초기에는:

```text
PostgreSQL + pgvector
```

로 단순화하고,

수십억 embedding scale로 가면 Qdrant 같은 별도 vector store를 분리합니다.

---

## 12. Source Analysis Engine

이 부분도 Rust가 상당히 좋은 선택입니다.

구조는:

```text
Git Repository
     ↓
Parser
     ↓
AST
     ↓
Symbol
     ↓
Call Graph
     ↓
Dependency Graph
```

언어별 parser는:

```text
tree-sitter
```

기반이 가장 현실적입니다.

예:

```text
COBOL
Java
C#
JavaScript
SQL
Python
C/C++
```

을 공통 AST abstraction으로 올립니다.

그리고 내부에:

```rust
struct CodeEntity {
    id: EntityId,
    language: Language,
    symbol: String,
    location: SourceLocation,
    dependencies: Vec<EntityId>,
}
```

형태의 canonical model을 둡니다.

---

## 13. LLM Gateway를 독립 서비스로 만드세요

Codex/Claude 호출을 각 Agent가 직접 하지 않게 해야 합니다.

구조:

```text
Agents
   │
   ↓
LLM Gateway
   │
   ├── OpenAI
   ├── Anthropic
   ├── AWS Bedrock
   └── Local Model
```

LLM Gateway가 관리:

```text
Model routing
Prompt version
Context assembly
Token budget
Caching
Retries
Rate limits
PII filtering
Audit
Cost
Evaluation
```

이 layer는 Rust + Axum/Tonic으로 만들면 좋습니다.

그리고 Agent는:

```rust
llm.complete(Task::BusinessRuleAnalysis(...))
```

만 호출합니다.

이것이 vendor lock-in을 크게 줄여줍니다.

---

## 14. Context Engine

LLM Gateway보다 더 중요한 부분입니다.

Agent에게 repository 전체를 넣지 않고:

```text
Task
  ↓
Knowledge Graph Query
  +
Tantivy Search
  +
Vector Retrieval
  +
Runtime Evidence
  ↓
Context Pack
```

을 만듭니다.

`ContextPack`은 예를 들면:

```rust
struct ContextPack {
    requirements: Vec<Requirement>,
    rules: Vec<BusinessRule>,
    source: Vec<CodeSnippet>,
    schema: Vec<SchemaEntity>,
    behaviors: Vec<BehaviorEvidence>,
    tests: Vec<TestEvidence>,
}
```

입니다.

이게 agent 정확도를 크게 좌우합니다.

---

## 15. Uncertainty Engine

이것 역시 Rust 서비스로 별도 구현하는 것을 권합니다.

```rust
struct ConfidenceVector {
    requirement: f64,
    rule: f64,
    behavior: f64,
    test: f64,
    dependency: f64,
    equivalence: f64,
}
```

그리고:

```text
Residual Risk
Confidence
Coverage
Evidence freshness
Agent disagreement
```

을 계산합니다.

중요한 점은 **LLM이 “confidence 95%”라고 말한 숫자를 그대로 쓰면 안 된다는 것**입니다.

점수는 evidence 기반이어야 합니다.

예:

```text
Production evidence exists      +0.25
Source + production agree       +0.20
2 models agree                  +0.05
Boundary tests passed           +0.10
Mutation survived               -0.30
Unexplained diff exists         -0.50
```

같이 deterministic scoring을 만듭니다.

---

## 16. Observability

기본은:

```text
tracing
+
OpenTelemetry
+
Prometheus
+
Grafana
```

입니다.

다만 현재 OpenTelemetry Rust는 traces / metrics / logs 모두 공식 문서 기준 아직 **Beta** 상태입니다. 

그래서 내부 instrumentation API를 OTel에 직접 과도하게 묶기보다는:

```text
tracing
    ↓
OTel adapter
```

식으로 abstraction을 두는 게 안전합니다.

---

## 17. Security

최소:

```text
mTLS
OIDC
Cedar
Secrets Manager / Vault
KMS
PII Tokenization
Immutable Audit Log
```

가 필요합니다.

특히 Production replay에서는 고객 데이터가 LLM으로 흘러가면 안 됩니다.

구조:

```text
Raw Production Data
        ↓
Tokenization / Masking
        ↓
Verification Evidence
        ↓
Context Filter
        ↓
LLM
```

로 두는 것이 좋습니다.

---

## 18. 배포 플랫폼

AWS 중심이라면:

```text
EKS
S3
Aurora PostgreSQL
MSK / Redpanda
ElastiCache
ECR
Secrets Manager
KMS
OpenSearch(optional)
```

정도가 좋습니다.

하지만 AMAP 같은 플랫폼 자체는 Kubernetes-compatible하게 유지합니다.

```text
AWS
Azure
GCP
On-prem
```

모두 가능하게 합니다.

대형 금융/공공 SI에서는 on-prem requirement가 반드시 생길 가능성이 높기 때문입니다.

---

## 19. 제가 추천하는 최종 Stack

### Core

| Layer | 추천 |
|---|---|
| Language | **Rust** |
| Runtime | **Tokio** |
| HTTP | **Axum** |
| RPC | **Tonic gRPC** |
| Middleware | Tower |
| Serialization | Serde |
| DB | PostgreSQL |
| Rust DB client | **SQLx** |
| Event Control | **NATS JetStream** |
| Streaming/Data Plane | **Redpanda** |
| Data format | **Apache Arrow / Parquet** |
| Evidence query | **DataFusion** |
| Object storage | S3 / MinIO |
| Search | Tantivy |
| Vector | pgvector → Qdrant |
| Graph | PostgreSQL canonical + petgraph |
| Policy | **Cedar** |
| Plugin sandbox | **Wasmtime** |
| AST | tree-sitter |
| Observability | tracing + OpenTelemetry |
| Metrics | Prometheus |
| Dashboard | Grafana |
| Workflow | Temporal 또는 자체 Rust abstraction |
| Deployment | Kubernetes |

---

## 20. Rust Workspace도 이렇게 자르는 것을 권합니다

```text
amap/
│
├── crates/
│   ├── domain/
│   ├── evidence/
│   ├── knowledge/
│   ├── graph/
│   ├── context/
│   ├── uncertainty/
│   ├── policy/
│   ├── comparator/
│   ├── invariant/
│   ├── replay/
│   └── telemetry/
│
├── services/
│   ├── control-plane/
│   ├── llm-gateway/
│   ├── orchestrator/
│   ├── discovery/
│   ├── behavior-miner/
│   ├── rule-miner/
│   ├── builder/
│   ├── verifier/
│   ├── rca/
│   ├── repair/
│   └── evidence-api/
│
├── workers/
│   ├── replay-worker/
│   ├── mutation-worker/
│   ├── fault-worker/
│   ├── concurrency-worker/
│   └── comparator-worker/
│
├── plugins/
│   ├── finance/
│   ├── manufacturing/
│   └── public/
│
└── proto/
```

**처음부터 50개 microservice로 만들지는 마세요.** Rust monorepo 안에서 modular monolith + worker 구조로 시작해서, workload가 명확히 갈리는 부분만 분리하는 편이 훨씬 낫습니다.

---

### 제가 특히 강하게 추천하는 5개 기술

전부 나열한 것보다 실제로는 이 다섯 가지가 플랫폼의 성격을 결정합니다.

**① Rust + Tokio** — deterministic하고 고성능인 검증 execution core.

**② Arrow + Parquet + DataFusion** — 수십억 건의 production replay/evidence 분석.

**③ Wasmtime** — 고객/산업별 비교 규칙을 안전한 plugin으로 만드는 기반.

**④ NATS/Redpanda** — Agent control과 massive behavior stream의 분리.

**⑤ PostgreSQL 중심 Evidence/Knowledge Model** — LLM의 기억이 아니라 **증거 자체를 시스템의 source of truth로 만드는 것**.

이렇게 가면 Rust는 단순히 “성능이 좋은 언어”가 아니라, **LLM처럼 확률적인 구성요소 주변을 deterministic한 검증 시스템으로 감싸는 언어** 역할을 하게 됩니다. 이 프로젝트에서는 그 특성이 상당히 잘 맞습니다.