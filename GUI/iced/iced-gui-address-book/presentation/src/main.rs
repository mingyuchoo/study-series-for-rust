//! 주소록 GUI 의 엔트리포인트.
//!
//! 상태·메시지·업데이트·뷰는 관심사별 모듈로 분리되어 있다.
//! - `app` — 상태 struct 와 합성 루트
//! - `message` — 사용자/비동기 이벤트를 나타내는 메시지
//! - `update` — 상태 전이와 키보드 구독
//! - `view` — 렌더링
//! - `theme` — DESIGN.md 디자인 토큰과 위젯 스타일

mod app;
mod message;
mod theme;
mod update;
mod view;

use app::AddressBook;

const NOTO_SANS_KR: &[u8] = include_bytes!("../fonts/NotoSansKR-Regular.ttf");

fn main() -> iced::Result {
    iced::application(AddressBook::new, AddressBook::update, AddressBook::view)
        .subscription(AddressBook::subscription)
        .title("Address Book")
        .font(NOTO_SANS_KR)
        .default_font(theme::REGULAR)
        .theme(iced::Theme::Light)
        .style(theme::application)
        .run()
}
