//! 렌더링(`view`)과 보조 뷰 함수들.
//!
//! 모든 색상·치수·스타일은 [`crate::theme`] 토큰만 참조한다(뷰와 디자인 토큰의
//! 분리).

use crate::{app::AddressBook,
            message::Message,
            theme};
use domain::entities::Address;
use iced::{Element,
           Length,
           widget::{Column,
                    Row,
                    button,
                    column,
                    container,
                    row,
                    scrollable,
                    text,
                    text_input}};

impl AddressBook {
    /// 현재 상태를 화면 요소로 렌더링한다.
    pub(crate) fn view(&self) -> Element<'_, Message> {
        let header = column![
            badge_chip("CONTACTS"),
            text("Address Book").size(theme::HEADING_1).font(theme::BOLD).color(theme::INK),
            text("Manage your contacts").size(theme::BODY_MD).color(theme::INK_MUTED),
        ]
        .spacing(theme::SP_XS);

        let mut content = Column::new().spacing(theme::SP_LG).push(header);

        if let Some(error) = &self.error_message {
            content = content.push(error_banner(error));
        }

        let list_heading = row![
            text("Saved Addresses").size(theme::HEADING_2).font(theme::BOLD).color(theme::INK),
            text(format!("({})", self.addresses.len())).size(theme::BODY_SM).color(theme::INK_FAINT),
        ]
        .spacing(theme::SP_SM);

        content = content
            .push(self.form_card())
            .push(self.action_buttons())
            .push(list_heading)
            .push(scrollable(self.address_list()).height(Length::Fill));

        let page = container(content).max_width(theme::CONTENT_MAX_WIDTH);

        container(page).padding(theme::SP_XXL).center_x(Length::Fill).height(Length::Fill).into()
    }

    /// 이름/전화/이메일/주소 입력 폼(흰색 카드).
    fn form_card(&self) -> Element<'_, Message> {
        let form = column![
            field("Name", "Enter name", &self.name_input, "name", Message::NameChanged),
            field("Phone", "Enter phone", &self.phone_input, "phone", Message::PhoneChanged),
            field("Email", "Enter email", &self.email_input, "email", Message::EmailChanged),
            field("Address", "Enter address", &self.address_input, "address", Message::AddressChanged),
        ]
        .spacing(theme::SP_MD);

        container(form).style(theme::card).padding(theme::SP_LG).width(Length::Fill).into()
    }

    /// 편집 모드면 Update/Cancel, 아니면 Add 버튼.
    fn action_buttons(&self) -> Element<'_, Message> {
        if self.editing_id.is_some() {
            row![
                button(text("Update").size(theme::BODY_MD).font(theme::MEDIUM))
                    .on_press(Message::UpdateAddress)
                    .padding([10, 24])
                    .style(theme::primary_button),
                button(text("Cancel").size(theme::BODY_MD).font(theme::MEDIUM))
                    .on_press(Message::CancelEdit)
                    .padding([10, 24])
                    .style(theme::secondary_button),
            ]
            .spacing(theme::SP_SM)
            .into()
        } else {
            row![
                button(text("Add").size(theme::BODY_MD).font(theme::MEDIUM))
                    .on_press(Message::CreateAddress)
                    .padding([10, 24])
                    .style(theme::primary_button)
            ]
            .into()
        }
    }

    /// 저장된 주소 목록.
    fn address_list(&self) -> Element<'_, Message> {
        self.addresses
            .iter()
            .fold(Column::new().spacing(theme::SP_MD), |col, addr| col.push(address_card(addr)))
            .into()
    }
}

/// eyebrow 배지 칩.
fn badge_chip(label: &str) -> Element<'_, Message> {
    container(text(label).size(theme::EYEBROW).font(theme::SEMIBOLD).color(theme::PRIMARY))
        .style(theme::badge)
        .padding([4, 8])
        .into()
}

/// 라벨 + 입력 한 줄.
fn field<'a>(label: &'a str, placeholder: &'a str, value: &'a str, id: &'static str, on_input: impl Fn(String) -> Message + 'a) -> Element<'a, Message> {
    column![
        text(label).size(theme::EYEBROW).font(theme::SEMIBOLD).color(theme::INK_MUTED),
        text_input(placeholder, value)
            .id(id)
            .on_input(on_input)
            .padding([8, 12])
            .size(theme::BODY_SM)
            .style(theme::input),
    ]
    .spacing(theme::SP_XS)
    .into()
}

/// 에러 배너(상태 표시).
fn error_banner(message: &str) -> Element<'_, Message> {
    container(text(format!("Error: {message}")).size(theme::BODY_SM).color(theme::ACCENT_ORANGE_DEEP))
        .style(theme::error_banner)
        .padding([theme::SP_SM, theme::SP_MD])
        .width(Length::Fill)
        .into()
}

/// "라벨: 값" 한 줄(라벨 폭 고정으로 정렬).
fn detail_row<'a>(label: &'a str, value: &'a str) -> Element<'a, Message> {
    row![
        text(label).size(theme::BODY_SM).color(theme::INK_FAINT).width(Length::Fixed(72.0)),
        text(value).size(theme::BODY_SM).color(theme::INK_SECONDARY),
    ]
    .spacing(theme::SP_XS)
    .into()
}

/// 주소 한 건을 카드 형태로 렌더링한다.
fn address_card(addr: &Address) -> Element<'_, Message> {
    // Edit 은 항상, Delete 는 영속화된(id 가 있는) 항목에만 제공한다.
    // 이전 구현의 `addr.id.unwrap()` 패닉을 제거한다.
    let mut actions = Row::new().spacing(theme::SP_XS).push(
        button(text("Edit").size(theme::CAPTION).font(theme::MEDIUM))
            .on_press(Message::EditAddress(addr.clone()))
            .padding([6, 14])
            .style(theme::utility_button),
    );
    if let Some(id) = addr.id {
        actions = actions.push(
            button(text("Delete").size(theme::CAPTION).font(theme::MEDIUM))
                .on_press(Message::DeleteAddress(id))
                .padding([6, 14])
                .style(theme::utility_button),
        );
    }

    let details = column![
        text(&addr.name).size(theme::TITLE).font(theme::SEMIBOLD).color(theme::INK),
        detail_row("Phone", &addr.phone),
        detail_row("Email", &addr.email),
        detail_row("Address", &addr.address),
    ]
    .spacing(theme::SP_XS);

    container(column![details, actions].spacing(theme::SP_MD))
        .style(theme::card)
        .padding(theme::SP_LG)
        .width(Length::Fill)
        .into()
}
