//! 렌더링(`view`)과 보조 뷰 함수들.

use crate::{app::AddressBook,
            message::Message};
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
        let mut content = Column::new().spacing(20).padding(20).push(text("Address Book").size(32));

        if let Some(error) = &self.error_message {
            content = content.push(text(format!("Error: {error}")).size(16));
        }

        content = content
            .push(self.input_form())
            .push(self.action_buttons())
            .push(text("Saved Addresses").size(24))
            .push(scrollable(self.address_list()).height(Length::Fill));

        container(content).width(Length::Fill).height(Length::Fill).into()
    }

    /// 이름/전화/이메일/주소 입력 폼.
    fn input_form(&self) -> Element<'_, Message> {
        column![
            text("Name:").size(16),
            text_input("Enter name", &self.name_input).id("name").on_input(Message::NameChanged).padding(10),
            text("Phone:").size(16),
            text_input("Enter phone", &self.phone_input)
                .id("phone")
                .on_input(Message::PhoneChanged)
                .padding(10),
            text("Email:").size(16),
            text_input("Enter email", &self.email_input)
                .id("email")
                .on_input(Message::EmailChanged)
                .padding(10),
            text("Address:").size(16),
            text_input("Enter address", &self.address_input)
                .id("address")
                .on_input(Message::AddressChanged)
                .padding(10),
        ]
        .spacing(10)
        .padding(20)
        .into()
    }

    /// 편집 모드면 Update/Cancel, 아니면 Add 버튼.
    fn action_buttons(&self) -> Element<'_, Message> {
        if self.editing_id.is_some() {
            row![
                button("Update").on_press(Message::UpdateAddress).padding(10),
                button("Cancel").on_press(Message::CancelEdit).padding(10),
            ]
            .spacing(10)
            .into()
        } else {
            row![button("Add").on_press(Message::CreateAddress).padding(10)].into()
        }
    }

    /// 저장된 주소 목록.
    fn address_list(&self) -> Element<'_, Message> {
        self.addresses
            .iter()
            .fold(Column::new().spacing(10), |col, addr| col.push(address_card(addr)))
            .into()
    }
}

/// 주소 한 건을 카드 형태로 렌더링한다.
fn address_card(addr: &Address) -> Element<'_, Message> {
    // Edit 은 항상, Delete 는 영속화된(id 가 있는) 항목에만 제공한다.
    // 이전 구현의 `addr.id.unwrap()` 패닉을 제거한다.
    let mut actions = Row::new()
        .spacing(10)
        .push(button("Edit").on_press(Message::EditAddress(addr.clone())).padding(5));
    if let Some(id) = addr.id {
        actions = actions.push(button("Delete").on_press(Message::DeleteAddress(id)).padding(5));
    }

    container(
        column![
            text(format!("Name: {}", addr.name)).size(18),
            text(format!("Phone: {}", addr.phone)).size(14),
            text(format!("Email: {}", addr.email)).size(14),
            text(format!("Address: {}", addr.address)).size(14),
            actions,
        ]
        .spacing(5)
        .padding(10),
    )
    .padding(10)
    .style(container::rounded_box)
    .into()
}
