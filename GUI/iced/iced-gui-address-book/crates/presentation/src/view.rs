//! 렌더링(`view`)과 보조 뷰 함수들.
//!
//! 모든 색상·치수·스타일은 [`crate::theme`] 토큰만 참조한다(뷰와 디자인 토큰의
//! 분리). Cal.com 디자인 시스템에 따라 상단 네비게이션, 모던 SaaS 폼 카드,
//! 파스텔 아바타 서클이 포함된 연락처 목록, 그리고 시그니처 다크 푸터를
//! 제공한다.

use crate::{app::AddressBook,
            i18n::{Language,
                   Strings},
            message::Message,
            theme};
use domain::entities::Address;
use iced::{Element,
           Length,
           widget::{Column,
                    Row,
                    Space,
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
        let strings = self.language.strings();
        let top_bar = top_nav(self);

        let hero_band = column![
            text(strings.app_title).size(theme::DISPLAY_SM).font(theme::SEMIBOLD).style(theme::text_ink),
            text(strings.hero_subtitle).size(theme::BODY_MD).style(theme::text_muted),
        ]
        .spacing(theme::SP_XXS);

        let mut page_content = Column::new().spacing(theme::SP_LG).push(hero_band);

        if let Some(error) = &self.error_message {
            page_content = page_content.push(error_banner(&self.language.error_message(error), strings));
        }

        let contact_count_text = self.language.contacts_count(self.addresses.len());
        let list_heading = row![
            text(strings.directory).size(theme::TITLE_LG).font(theme::SEMIBOLD).style(theme::text_ink),
            badge_pill(contact_count_text),
        ]
        .spacing(theme::SP_SM)
        .align_y(iced::Alignment::Center);

        page_content = page_content.push(self.form_card()).push(list_heading).push(self.address_list());

        let page_column = container(page_content)
            .max_width(theme::CONTENT_MAX_WIDTH)
            .width(Length::Fill)
            .padding([theme::SP_LG, theme::SP_LG]);

        let scroll_body = scrollable(container(page_column).center_x(Length::Fill).width(Length::Fill)).height(Length::Fill);

        column![top_bar, scroll_body, dark_footer(strings)].into()
    }

    /// 이름/전화/이메일/주소 입력 폼(화이트 캔버스 목업 카드).
    fn form_card(&self) -> Element<'_, Message> {
        let strings = self.language.strings();
        let is_editing = self.editing_id.is_some();

        let header = column![
            badge_pill(if is_editing { strings.badge_editing } else { strings.badge_new }),
            text(if is_editing { strings.form_title_edit } else { strings.form_title_new })
                .size(theme::TITLE_MD)
                .font(theme::SEMIBOLD)
                .style(theme::text_ink),
            text(if is_editing { strings.form_desc_edit } else { strings.form_desc_new })
                .size(theme::BODY_SM)
                .style(theme::text_muted),
        ]
        .spacing(theme::SP_XS);

        let row1 = row![
            field(strings.label_name, strings.placeholder_name, &self.name_input, "name", Message::NameChanged),
            field(
                strings.label_phone,
                strings.placeholder_phone,
                &self.phone_input,
                "phone",
                Message::PhoneChanged
            ),
        ]
        .spacing(theme::SP_MD);

        let row2 = row![
            field(
                strings.label_email,
                strings.placeholder_email,
                &self.email_input,
                "email",
                Message::EmailChanged
            ),
            field(
                strings.label_address,
                strings.placeholder_address,
                &self.address_input,
                "address",
                Message::AddressChanged
            ),
        ]
        .spacing(theme::SP_MD);

        let form = column![header, row1, row2, self.action_buttons(),].spacing(theme::SP_MD);

        container(form).style(theme::card).padding(theme::SP_LG).width(Length::Fill).into()
    }

    /// 편집 모드면 Update/Cancel, 아니면 Add 버튼.
    fn action_buttons(&self) -> Element<'_, Message> {
        let strings = self.language.strings();
        if self.editing_id.is_some() {
            row![
                button(text(strings.button_save).size(theme::BUTTON_TEXT).font(theme::SEMIBOLD))
                    .on_press(Message::UpdateAddress)
                    .padding([10, 20])
                    .style(theme::primary_button),
                button(text(strings.button_cancel).size(theme::BUTTON_TEXT).font(theme::SEMIBOLD))
                    .on_press(Message::CancelEdit)
                    .padding([10, 20])
                    .style(theme::secondary_button),
            ]
            .spacing(theme::SP_SM)
            .into()
        } else {
            row![
                button(text(strings.button_add).size(theme::BUTTON_TEXT).font(theme::SEMIBOLD))
                    .on_press(Message::CreateAddress)
                    .padding([10, 20])
                    .style(theme::primary_button)
            ]
            .into()
        }
    }

    /// 저장된 주소 목록 또는 빈 상태 안내.
    fn address_list(&self) -> Element<'_, Message> {
        if self.addresses.is_empty() {
            empty_state(self.language.strings())
        } else {
            self.addresses
                .iter()
                .fold(Column::new().spacing(theme::SP_SM), |col, addr| {
                    col.push(address_card(addr, self.language.strings()))
                })
                .into()
        }
    }
}

/// 상단 네비게이션 바.
fn top_nav(app: &AddressBook) -> Element<'static, Message> {
    let strings = app.language.strings();
    let brand = row![
        text(strings.app_title).size(theme::TITLE_MD).font(theme::SEMIBOLD).style(theme::text_ink),
        badge_pill(strings.nav_badge),
        Space::new().width(Length::Fill),
        badge_pill(app.language.contacts_count(app.addresses.len())),
    ]
    .spacing(theme::SP_SM)
    .align_y(iced::Alignment::Center);

    let languages = Language::ALL.into_iter().fold(Row::new(), |group, language| {
        group.push(
            button(text(language.label()).size(theme::NAV_LINK).font(theme::MEDIUM))
                .on_press(Message::LanguageSelected(language))
                .padding([8, 14])
                .height(40)
                .style(theme::category_tab(app.language == language)),
        )
    });
    let themes = theme::ThemeMode::ALL.into_iter().fold(Row::new(), |group, mode| {
        group.push(
            button(text(mode.label(strings)).size(theme::NAV_LINK).font(theme::MEDIUM))
                .on_press(Message::ThemeSelected(mode))
                .padding([8, 14])
                .height(40)
                .style(theme::category_tab(app.theme_mode == mode)),
        )
    });
    // Keep preferences on their own wrapping row so all five options fit at
    // 640px.
    let preferences = row![
        preference_group(strings.language_label, languages),
        preference_group(strings.theme_label, themes),
    ]
    .spacing(theme::SP_MD)
    .wrap();

    let content = container(column![brand, preferences].spacing(theme::SP_SM))
        .width(Length::Fill)
        .max_width(theme::CONTENT_MAX_WIDTH);
    container(content)
        .style(theme::top_nav)
        .padding([theme::SP_SM, theme::SP_LG])
        .center_x(Length::Fill)
        .width(Length::Fill)
        .into()
}

fn preference_group(label: &'static str, segments: Row<'static, Message>) -> Element<'static, Message> {
    row![
        text(label).size(theme::CAPTION).font(theme::MEDIUM).style(theme::text_muted),
        container(segments).padding(6).style(theme::nav_pill_group),
    ]
    .spacing(theme::SP_XS)
    .align_y(iced::Alignment::Center)
    .into()
}

/// Cal.com 다크 푸터(화면을 닫아주는 딥 네이비/블랙 surface-dark 밴드).
fn dark_footer(strings: &'static Strings) -> Element<'static, Message> {
    let footer_content = column![
        text(strings.footer_title).size(theme::BODY_SM).font(theme::SEMIBOLD).color(theme::ON_DARK),
        text(strings.footer_desc).size(theme::CAPTION).color(theme::ON_DARK_SOFT),
    ]
    .spacing(theme::SP_XXS)
    .align_x(iced::Alignment::Center);

    container(footer_content)
        .style(theme::footer)
        .padding([theme::SP_LG, theme::SP_LG])
        .center_x(Length::Fill)
        .width(Length::Fill)
        .into()
}

/// 주소록이 비어있을 때 표시되는 빈 상태 카드.
fn empty_state(strings: &'static Strings) -> Element<'static, Message> {
    let content = column![
        text(strings.empty_title).size(theme::TITLE_MD).font(theme::SEMIBOLD).style(theme::text_ink),
        text(strings.empty_desc).size(theme::BODY_SM).style(theme::text_muted),
    ]
    .spacing(theme::SP_XS)
    .align_x(iced::Alignment::Center);

    container(content)
        .style(theme::contact_card)
        .padding([theme::SP_LG, theme::SP_LG])
        .center_x(Length::Fill)
        .width(Length::Fill)
        .into()
}

/// badge-pill 위젯.
fn badge_pill<'a>(label: impl iced::widget::text::IntoFragment<'a>) -> Element<'a, Message> {
    container(text(label).size(theme::CAPTION).font(theme::MEDIUM).style(theme::text_ink))
        .style(theme::badge_pill)
        .padding([4, 10])
        .into()
}

/// 이름에서 대표 이니셜 한 글자를 추출한다.
fn get_initial(name: &str) -> String {
    name.trim()
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

/// 36px 원형 파스텔 아바타 위젯.
fn avatar(name: &str) -> Element<'_, Message> {
    let color = theme::avatar_color_for_name(name);
    let initial = get_initial(name);

    container(text(initial).size(theme::BODY_SM).font(theme::SEMIBOLD).color(theme::ON_PRIMARY))
        .style(theme::avatar_circle(color))
        .width(Length::Fixed(36.0))
        .height(Length::Fixed(36.0))
        .center_x(Length::Fixed(36.0))
        .center_y(Length::Fixed(36.0))
        .into()
}

/// 라벨 + 40px 입력창 컬럼.
fn field<'a>(label: &'a str, placeholder: &'a str, value: &'a str, id: &'static str, on_input: impl Fn(String) -> Message + 'a) -> Element<'a, Message> {
    column![
        text(label).size(theme::CAPTION).font(theme::SEMIBOLD).style(theme::text_muted),
        text_input(placeholder, value)
            .id(id)
            .on_input(on_input)
            .padding([10, 14])
            .size(theme::BODY_SM)
            .style(theme::input),
    ]
    .spacing(theme::SP_XS)
    .width(Length::FillPortion(1))
    .into()
}

/// 에러 배너(상태 표시).
fn error_banner(message: &str, strings: &'static Strings) -> Element<'static, Message> {
    container(text(format!("{}: {message}", strings.error_prefix)).size(theme::BODY_SM).color(theme::ERROR))
        .style(theme::error_banner)
        .padding([theme::SP_SM, theme::SP_MD])
        .width(Length::Fill)
        .into()
}

/// 주소 한 건을 Cal.com 스타일 카드 형태로 렌더링한다.
fn address_card<'a>(addr: &'a Address, strings: &'static Strings) -> Element<'a, Message> {
    let mut actions = Row::new().spacing(theme::SP_XS).push(
        button(text(strings.button_edit).size(theme::CAPTION).font(theme::MEDIUM))
            .on_press(Message::EditAddress(addr.clone()))
            .padding([6, 14])
            .style(theme::utility_button),
    );
    if let Some(id) = addr.id {
        actions = actions.push(
            button(text(strings.button_delete).size(theme::CAPTION).font(theme::MEDIUM))
                .on_press(Message::DeleteAddress(id))
                .padding([6, 14])
                .style(theme::delete_button),
        );
    }

    let meta_row = row![
        text(format!("{}: {}", strings.meta_phone, if addr.phone.is_empty() { "—" } else { &addr.phone }))
            .size(theme::BODY_SM)
            .style(theme::text_body),
        text("•").size(theme::BODY_SM).style(theme::text_muted_soft),
        text(format!("{}: {}", strings.meta_email, if addr.email.is_empty() { "—" } else { &addr.email }))
            .size(theme::BODY_SM)
            .style(theme::text_muted),
    ]
    .spacing(theme::SP_XS)
    .align_y(iced::Alignment::Center);

    let mut info_col = column![text(&addr.name).size(theme::TITLE_MD).font(theme::SEMIBOLD).style(theme::text_ink), meta_row,].spacing(theme::SP_XXS);

    if !addr.address.is_empty() {
        info_col = info_col.push(
            text(format!("{}: {}", strings.meta_address, addr.address))
                .size(theme::CAPTION)
                .style(theme::text_muted),
        );
    }

    let card_content = row![avatar(&addr.name), info_col.width(Length::Fill), actions.align_y(iced::Alignment::Center),]
        .spacing(theme::SP_MD)
        .align_y(iced::Alignment::Center);

    container(card_content)
        .style(theme::contact_card)
        .padding(theme::SP_MD)
        .width(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use application::usecases::AddressUseCases;
    use domain::{error::RepositoryError,
                 repositories::AddressRepository};
    use std::sync::Arc;

    struct EmptyRepository;
    impl AddressRepository for EmptyRepository {
        fn create(&self, address: Address) -> Result<Address, RepositoryError> { Ok(address) }

        fn read(&self, _id: i64) -> Result<Option<Address>, RepositoryError> { Ok(None) }

        fn read_all(&self) -> Result<Vec<Address>, RepositoryError> { Ok(Vec::new()) }

        fn update(&self, address: Address) -> Result<Address, RepositoryError> { Ok(address) }

        fn delete(&self, _id: i64) -> Result<(), RepositoryError> { Ok(()) }
    }

    fn app() -> AddressBook {
        AddressBook {
            usecases: Arc::new(AddressUseCases::new(Arc::new(EmptyRepository))),
            addresses: Vec::new(),
            name_input: String::new(),
            phone_input: String::new(),
            email_input: String::new(),
            address_input: String::new(),
            editing_id: None,
            error_message: None,
            language: Default::default(),
            theme_mode: Default::default(),
        }
    }

    #[test]
    fn view_builds_for_empty_error_and_editing_states() {
        let mut app = app();
        drop(app.view());
        app.error_message = Some(application::error::AppError::Validation(domain::error::ValidationError::EmptyName));
        app.editing_id = Some(1);
        app.addresses = vec![
            Address {
                id: Some(1),
                name: "Alice".into(),
                phone: "010".into(),
                email: "a@b.com".into(),
                address: "Seoul".into(),
            },
            Address {
                id: None,
                name: "Draft".into(),
                phone: "011".into(),
                email: "d@b.com".into(),
                address: "Busan".into(),
            },
        ];
        drop(app.view());
    }
}
