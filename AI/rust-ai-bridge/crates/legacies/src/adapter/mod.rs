//! 레거시 시스템 어댑터 구현.
//!
//! 각 어댑터는 하나의 레거시 시스템을 **표준 도구 집합**으로 감쌉니다. 어댑터는
//! [`Transport`](crate::legacy::Transport) 뒤에서 프로토콜을 모르고,
//! 게이트웨이는 어댑터 뒤에서 데이터를 모릅니다.
//!
//! **행 수준 접근 제어는 어댑터가 합니다.** 정책 엔진은 "이 도구를 호출해도
//! 되는가"까지만 판단하고, 검색 결과에서 볼 수 없는 행을 걸러내는 일은 데이터를
//! 아는 쪽만 할 수 있습니다.

mod crm;
mod docs;
mod erp;
mod purchase;
mod refund;
mod ticket;

pub use crm::CrmAdapter;
pub use docs::DocsAdapter;
pub use erp::ErpAdapter;
pub use purchase::PurchaseAdapter;
pub use refund::{Ledger,
                 RefundAdapter};
pub use ticket::TicketAdapter;
