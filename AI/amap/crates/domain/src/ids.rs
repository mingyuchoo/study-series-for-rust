use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! typed_id {
    ($name:ident, $prefix:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub const PREFIX: &'static str = $prefix;
            pub fn new(raw: impl Into<String>) -> Self {
                Self(raw.into())
            }
            /// Build `PREFIX-000123` style identifiers.
            pub fn numbered(n: u64) -> Self {
                Self(format!("{}-{:06}", $prefix, n))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }
        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }
    };
}

typed_id!(RequirementId, "REQ", "Requirement identifier (REQ-1203).");
typed_id!(FunctionId, "FN", "Business function identifier (FN-01882).");
typed_id!(RuleId, "BR", "Business rule identifier (BR-LOAN-000183).");
typed_id!(
    BehaviorId,
    "BH",
    "Production behavior record identifier (BH-92182012)."
);
typed_id!(ScenarioId, "TEST", "Test scenario identifier (TEST-88291).");
typed_id!(InterfaceId, "IF", "Interface identifier (IF-238).");
typed_id!(
    SourceUnitId,
    "SRC",
    "Source unit identifier (file / paragraph / method)."
);
typed_id!(
    DbEntityId,
    "DB",
    "Database entity identifier (table / column)."
);
typed_id!(RunId, "RUN", "Orchestration run identifier.");
typed_id!(DecisionId, "ADR", "Architecture decision identifier.");

/// A precise location in legacy or next-generation source (`LOAN231.cbl:2912-2947`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
}

impl SourceLocation {
    pub fn new(file: impl Into<String>, start_line: u32, end_line: u32) -> Self {
        Self {
            file: file.into(),
            start_line,
            end_line,
        }
    }
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}-{}", self.file, self.start_line, self.end_line)
    }
}

/// Business priority. P0 must be 100% equivalent; P1 ≥ 99.999%.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub enum Priority {
    P0,
    P1,
    #[default]
    P2,
    P3,
}

impl Priority {
    pub fn is_critical(self) -> bool {
        matches!(self, Priority::P0 | Priority::P1)
    }
}
