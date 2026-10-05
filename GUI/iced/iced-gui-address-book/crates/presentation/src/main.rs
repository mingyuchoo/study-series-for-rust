//! 주소록 GUI 의 엔트리포인트.
//!
//! 상태·메시지·업데이트·뷰는 관심사별 모듈로 분리되어 있다.
//! - `app` — 상태 struct 와 합성 루트
//! - `message` — 사용자/비동기 이벤트를 나타내는 메시지
//! - `update` — 상태 전이와 키보드 구독
//! - `view` — 렌더링
//! - `theme` — DESIGN.md 디자인 토큰과 위젯 스타일

mod app;
mod i18n;
mod message;
mod theme;
mod update;
mod view;

use app::AddressBook;

fn main() -> iced::Result {
    iced::application(AddressBook::new, AddressBook::update, AddressBook::view)
        .title(AddressBook::title)
        .subscription(AddressBook::subscription)
        .window(iced::window::Settings {
            size: iced::Size::new(980.0, 900.0),
            min_size: Some(iced::Size::new(640.0, 500.0)),
            position: iced::window::Position::Centered,
            ..Default::default()
        })
        .font(include_bytes!("../fonts/NotoSansKR-Regular.ttf").as_slice())
        .default_font(theme::REGULAR)
        .theme(AddressBook::theme)
        .style(theme::application)
        .run()
}
