//! 대시보드 탭 문구.

use super::{Lang,
            pick};

impl Lang {
    // ── 대시보드 ─────────────────────────────────────────────
    pub fn dash_filter_notice(self) -> &'static str { pick(self, "검색 결과 기준 집계입니다.", "Aggregated from search results.") }

    pub fn all_kpis(self) -> &'static str { pick(self, "전체 핵심 성과 지표", "All Key Performance Indicators") }

    pub fn kpi_levels_heading(self) -> &'static str { pick(self, "핵심 성과 지표 현재 수준", "Key Performance Indicator current levels") }

    pub fn dash_average(self, average: i64) -> String {
        match self {
            | Self::Ko => format!("평균 달성률 {average}%"),
            | Self::En => format!("Average progress {average}%"),
        }
    }

    pub fn dash_no_measurable(self) -> &'static str {
        pick(
            self,
            "달성률을 계산할 핵심 성과 지표가 없습니다.",
            "No Key Performance Indicators with a computable progress.",
        )
    }

    pub fn dash_no_target_suffix(self, count: usize) -> String {
        match self {
            | Self::Ko => format!(" · 목표 미설정 {count}개"),
            | Self::En => format!(" · {count} without a target"),
        }
    }

    pub fn no_kpis(self) -> &'static str { pick(self, "핵심 성과 지표가 없습니다.", "No Key Performance Indicators.") }

    pub fn no_target(self) -> &'static str { pick(self, "목표 미설정", "No target") }
}
