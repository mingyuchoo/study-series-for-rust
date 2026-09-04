//! Versioned prompt templates per task. Prompts are data (version pinned) so the gateway can audit
//! which prompt version produced which artefact.
use crate::TaskKind;
use serde_json::{json, Value};

pub const PROMPT_VERSION: &str = "2026.09.1";

pub struct PromptTemplate {
    pub version: &'static str,
    pub system: &'static str,
    pub schema: Value,
}

const COMMON: &str = "You are a component of an evidence-driven legacy modernization platform. \
Be precise and literal. Cite source locations as file:start-end. Never claim certainty you cannot ground in the provided context; \
report unknowns explicitly. Respond with JSON matching the schema only.";

pub fn template(task: TaskKind) -> PromptTemplate {
    match task {
        TaskKind::DomainClassification => PromptTemplate {
            version: PROMPT_VERSION,
            system: "Classify legacy code entities into business domains and flag dead or suspicious code.",
            schema: json!({"type":"object","properties":{
                "domains":{"type":"array","items":{"type":"object","properties":{"entity":{"type":"string"},"domain":{"type":"string"}},"required":["entity","domain"],"additionalProperties":false}},
                "suspicious":{"type":"array","items":{"type":"string"}}},
                "required":["domains","suspicious"],"additionalProperties":false}),
        },
        TaskKind::BusinessRuleExtraction => PromptTemplate {
            version: PROMPT_VERSION,
            system: "Extract every business rule from the legacy source in the context. For each rule give a DSL condition and result \
                     (operators: == != < <= > >= AND OR NOT, dotted paths, functions SUM COUNT ABS ROUND), the exact source location, \
                     tables and interfaces touched, and which evidence kinds support it (code, document). Do NOT estimate confidence; the platform computes it.",
            schema: json!({"type":"object","properties":{"rules":{"type":"array","items":{"type":"object","properties":{
                "name":{"type":"string"},"condition":{"type":"string"},"result":{"type":"string"},
                "source":{"type":"object","properties":{"file":{"type":"string"},"start_line":{"type":"integer"},"end_line":{"type":"integer"}},"required":["file","start_line","end_line"],"additionalProperties":false},
                "db_entities":{"type":"array","items":{"type":"string"}},"interfaces":{"type":"array","items":{"type":"string"}},
                "requirement_ids":{"type":"array","items":{"type":"string"}},
                "in_document":{"type":"boolean"},"priority":{"type":"string","enum":["P0","P1","P2","P3"]}},
                "required":["name","condition","result","source","db_entities","interfaces","requirement_ids","in_document","priority"],"additionalProperties":false}}},
                "required":["rules"],"additionalProperties":false}),
        },
        TaskKind::BehaviorLinking => PromptTemplate {
            version: PROMPT_VERSION,
            system: "Given production behavior records and business rules, list which rules each behavior exercises.",
            schema: json!({"type":"object","properties":{"links":{"type":"array","items":{"type":"object","properties":{"behavior_id":{"type":"string"},"rule_ids":{"type":"array","items":{"type":"string"}}},"required":["behavior_id","rule_ids"],"additionalProperties":false}}},"required":["links"],"additionalProperties":false}),
        },
        TaskKind::ArchitectureMapping => PromptTemplate {
            version: PROMPT_VERSION,
            system: "Decide how the legacy function maps to the target architecture. Do not write code. Give the decision, evidence, alternatives, risks, affected rules and tests, and rule → service ownership.",
            schema: json!({"type":"object","properties":{"decision":{"type":"string"},"evidence":{"type":"array","items":{"type":"string"}},
                "alternatives":{"type":"array","items":{"type":"string"}},"risks":{"type":"array","items":{"type":"string"}},
                "affected_rules":{"type":"array","items":{"type":"string"}},"affected_tests":{"type":"array","items":{"type":"string"}},
                "rule_ownership":{"type":"array","items":{"type":"object","properties":{"rule_id":{"type":"string"},"service":{"type":"string"}},"required":["rule_id","service"],"additionalProperties":false}}},
                "required":["decision","evidence","alternatives","risks","affected_rules","affected_tests","rule_ownership"],"additionalProperties":false}),
        },
        TaskKind::CodeGeneration => PromptTemplate {
            version: PROMPT_VERSION,
            system: "Implement the bounded context described. Preserve every business rule exactly, including legacy rounding and day-count conventions. \
                     Output complete files. The program must read JSON lines from stdin ({\"initial_state\":…,\"input\":…,\"options\":…}) and write one JSON line per case \
                     ({\"output\":…,\"state_change\":…,\"events\":[…],\"external_calls\":[…]}).",
            schema: json!({"type":"object","properties":{"files":{"type":"array","items":{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"],"additionalProperties":false}},"notes":{"type":"string"}},"required":["files","notes"],"additionalProperties":false}),
        },
        TaskKind::TestGeneration => PromptTemplate {
            version: PROMPT_VERSION,
            system: "Generate independent test scenarios (inputs + expected outputs where derivable from rules) for the function. You have not seen the implementation.",
            schema: json!({"type":"object","properties":{"scenarios":{"type":"array","items":{"type":"object","properties":{"name":{"type":"string"},"rule_ids":{"type":"array","items":{"type":"string"}},"initial_state":{"type":"object"},"input":{"type":"object"},"expected_output":{"type":["object","null"]}},"required":["name","rule_ids","initial_state","input","expected_output"],"additionalProperties":false}}},"required":["scenarios"],"additionalProperties":false}),
        },
        TaskKind::AdversarialProbe => PromptTemplate {
            version: PROMPT_VERSION,
            system: "Your only question: how can this system be broken? Produce hostile inputs (timezone, leap year, month/year end, duplicates, out-of-order events, negative/max values, encoding, null, partial failure, retry, concurrent update). You are independent from the builder.",
            schema: json!({"type":"object","properties":{"probes":{"type":"array","items":{"type":"object","properties":{"name":{"type":"string"},"category":{"type":"string"},"initial_state":{"type":"object"},"input":{"type":"object"},"rule_ids":{"type":"array","items":{"type":"string"}},"expectation":{"type":"string"}},"required":["name","category","initial_state","input","rule_ids","expectation"],"additionalProperties":false}}},"required":["probes"],"additionalProperties":false}),
        },
        TaskKind::RootCauseAnalysis => PromptTemplate {
            version: PROMPT_VERSION,
            system: "Analyse failed scenarios (legacy vs next traces, diffs, relevant rules and source). Produce a root-cause hypothesis with location and the legacy vs next behaviour. Do not propose a patch.",
            schema: json!({"type":"object","properties":{"summary":{"type":"string"},"location":{"type":["object","null"],"properties":{"file":{"type":"string"},"start_line":{"type":"integer"},"end_line":{"type":"integer"}},"required":["file","start_line","end_line"],"additionalProperties":false},
                "legacy_behavior":{"type":"string"},"next_behavior":{"type":"string"},"affected_rules":{"type":"array","items":{"type":"string"}}},
                "required":["summary","location","legacy_behavior","next_behavior","affected_rules"],"additionalProperties":false}),
        },
        TaskKind::PatchGeneration => PromptTemplate {
            version: PROMPT_VERSION,
            system: "Given a root-cause hypothesis and the current source, produce a minimal patch as complete replacement files. Explain the rationale.",
            schema: json!({"type":"object","properties":{"rationale":{"type":"string"},"files":{"type":"array","items":{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"],"additionalProperties":false}}},"required":["rationale","files"],"additionalProperties":false}),
        },
        TaskKind::CodeReview => PromptTemplate {
            version: PROMPT_VERSION,
            system: "Independently review the patch against the business rules and root cause. Approve only if the change is minimal, correct and preserves all other rules.",
            schema: json!({"type":"object","properties":{"approved":{"type":"boolean"},"comments":{"type":"array","items":{"type":"string"}}},"required":["approved","comments"],"additionalProperties":false}),
        },
        TaskKind::BusinessReview => PromptTemplate {
            version: PROMPT_VERSION,
            system: "Review from the business perspective: does the interpretation of the rules match the documented business intent?",
            schema: json!({"type":"object","properties":{"approved":{"type":"boolean"},"comments":{"type":"array","items":{"type":"string"}}},"required":["approved","comments"],"additionalProperties":false}),
        },
    }
}

pub fn system_prompt(task: TaskKind) -> String {
    format!("{COMMON}\n\n{}", template(task).system)
}
