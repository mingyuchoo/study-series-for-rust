//! 전체 구조 트리와 단계 탭 표 문구.

use super::{Lang,
            pick};

impl Lang {
    // ── 단계 탭 표 ───────────────────────────────────────────
    pub fn empty_lane(self) -> &'static str { pick(self, "비어 있음", "Empty") }

    // ── 전체 구조 트리 ───────────────────────────────────────
    pub fn tree_empty_hint(self) -> &'static str {
        pick(
            self,
            "아직 IKIK 항목이 없습니다. 아래에서 정체성부터 추가해 보세요.",
            "No IKIK items yet. Start by adding an Identity below.",
        )
    }

    pub fn expand_all(self) -> &'static str { pick(self, "모두 펼치기", "Expand all") }

    pub fn collapse_all(self) -> &'static str { pick(self, "모두 접기", "Collapse all") }

    pub fn tree_flow_hint(self) -> &'static str {
        pick(
            self,
            "정체성 → 핵심 결과 영역 → 소득 창출 업무 → 핵심 성과 지표 · 행을 끌어 새 상위 항목 위에 놓으면 이동합니다",
            "Identity → Key Result Area → Income Generating Task → Key Performance Indicator · drag a row onto a new parent to move it",
        )
    }

    pub fn add_identity(self) -> &'static str { pick(self, "+ 정체성 추가", "+ Add Identity") }

    pub fn collapse(self) -> &'static str { pick(self, "접기", "Collapse") }

    pub fn expand(self) -> &'static str { pick(self, "펼치기", "Expand") }

    pub fn nested_count(self, count: usize) -> String {
        match self {
            | Self::Ko => format!("하위 {count}"),
            | Self::En => format!("{count} nested"),
        }
    }
}
