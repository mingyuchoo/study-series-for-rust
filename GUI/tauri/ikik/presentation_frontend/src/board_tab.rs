//! 보드 탭 상태. 대시보드 / 전체 구조 / 단계별(Identity..Kpi) 중 하나다.
//!
//! 문자열("dashboard" 등) 대신 타입으로 들고 다녀, "탭 → 화면" 해석이
//! 보드 한곳에 모이고 매직 스트링이 사라진다. 모드·테마와 달리 보존
//! 대상이 아니라 세션 동안만 유지된다.

use crate::models::ItemKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoardTab {
    /// 단계별 수량·상태 분포·KPI 달성 현황을 모아 보여 주는 기본 탭.
    #[default]
    Dashboard,
    /// 전체 구조 트리.
    Structure,
    /// 한 단계만 모아 보는 탭.
    Kind(ItemKind),
}

impl BoardTab {
    /// 이 탭에서 「새 항목」을 열 때의 기본 단계. 단계 탭을 보고 있었다면
    /// 그 단계를, 그 밖에는 최상위 Identity를 기본 선택한다.
    pub fn default_new_kind(self) -> ItemKind {
        match self {
            | Self::Kind(kind) => kind,
            | _ => ItemKind::Identity,
        }
    }
}
