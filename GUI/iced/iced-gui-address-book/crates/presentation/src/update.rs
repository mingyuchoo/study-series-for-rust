//! 상태 전이(`update`)와 키보드 구독.

use crate::{app::AddressBook,
            message::Message};
use domain::entities::Address;
use iced::{Subscription,
           Task,
           keyboard::{self,
                      key},
           widget::operation::{focus_next,
                               focus_previous}};

impl AddressBook {
    /// 메시지를 받아 상태를 갱신하고 후속 비동기 태스크를 반환한다.
    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            | Message::NameChanged(value) => {
                self.name_input = value;
                Task::none()
            },
            | Message::PhoneChanged(value) => {
                self.phone_input = value;
                Task::none()
            },
            | Message::EmailChanged(value) => {
                self.email_input = value;
                Task::none()
            },
            | Message::AddressChanged(value) => {
                self.address_input = value;
                Task::none()
            },
            | Message::CreateAddress => {
                let usecases = self.usecases.clone();
                let name = self.name_input.clone();
                let phone = self.phone_input.clone();
                let email = self.email_input.clone();
                let address = self.address_input.clone();

                self.clear_inputs();

                Task::perform(
                    async move {
                        usecases.create_address(name, phone, email, address)?;
                        usecases.get_all_addresses()
                    },
                    Message::AddressesLoaded,
                )
            },
            | Message::DeleteAddress(id) => {
                let usecases = self.usecases.clone();
                Task::perform(
                    async move {
                        usecases.delete_address(id)?;
                        usecases.get_all_addresses()
                    },
                    Message::AddressesLoaded,
                )
            },
            | Message::EditAddress(address) => {
                self.editing_id = address.id;
                self.name_input = address.name;
                self.phone_input = address.phone;
                self.email_input = address.email;
                self.address_input = address.address;
                Task::none()
            },
            | Message::UpdateAddress =>
                if let Some(id) = self.editing_id {
                    let usecases = self.usecases.clone();
                    let address = Address {
                        id: Some(id),
                        name: self.name_input.clone(),
                        phone: self.phone_input.clone(),
                        email: self.email_input.clone(),
                        address: self.address_input.clone(),
                    };

                    self.clear_inputs();
                    self.editing_id = None;

                    Task::perform(
                        async move {
                            usecases.update_address(address)?;
                            usecases.get_all_addresses()
                        },
                        Message::AddressesLoaded,
                    )
                } else {
                    Task::none()
                },
            | Message::CancelEdit => {
                self.editing_id = None;
                self.clear_inputs();
                Task::none()
            },
            | Message::LoadAddresses => {
                let usecases = self.usecases.clone();
                Task::perform(async move { usecases.get_all_addresses() }, Message::AddressesLoaded)
            },
            | Message::AddressesLoaded(result) => {
                match result {
                    | Ok(addresses) => {
                        self.addresses = addresses;
                        self.error_message = None;
                    },
                    | Err(error) => self.error_message = Some(error),
                }
                Task::none()
            },
            | Message::TabPressed {
                shift,
            } =>
                if shift {
                    focus_previous()
                } else {
                    focus_next()
                },
            | Message::LanguageSelected(language) => {
                self.language = language;
                Task::none()
            },
            | Message::ThemeSelected(mode) => {
                self.theme_mode = mode;
                Task::none()
            },
        }
    }

    /// Tab / Shift+Tab 키로 입력 포커스를 이동하기 위한 구독.
    pub(crate) fn subscription(&self) -> Subscription<Message> {
        keyboard::listen().filter_map(|event| match event {
            | keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(key::Named::Tab),
                modifiers,
                ..
            } => Some(Message::TabPressed {
                shift: modifiers.shift(),
            }),
            | _ => None,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use application::{error::AppError,
                      usecases::AddressUseCases};
    use domain::{error::{RepositoryError,
                         ValidationError},
                 repositories::AddressRepository};
    use std::sync::Arc;

    struct EmptyRepository;

    impl AddressRepository for EmptyRepository {
        fn create(&self, mut address: Address) -> Result<Address, RepositoryError> {
            address.id = Some(1);
            Ok(address)
        }

        fn read(&self, _id: i64) -> Result<Option<Address>, RepositoryError> { Ok(None) }

        fn read_all(&self) -> Result<Vec<Address>, RepositoryError> { Ok(Vec::new()) }

        fn update(&self, address: Address) -> Result<Address, RepositoryError> { Ok(address) }

        fn delete(&self, _id: i64) -> Result<(), RepositoryError> { Ok(()) }
    }

    pub(crate) fn app_for_theme_test() -> AddressBook {
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

    fn app() -> AddressBook { app_for_theme_test() }

    fn address(id: Option<i64>) -> Address {
        Address {
            id,
            name: "Alice".into(),
            phone: "010".into(),
            email: "a@b.com".into(),
            address: "Seoul".into(),
        }
    }

    #[test]
    fn input_messages_update_each_field() {
        let mut app = app();
        drop(app.update(Message::NameChanged("Alice".into())));
        drop(app.update(Message::PhoneChanged("010".into())));
        drop(app.update(Message::EmailChanged("a@b.com".into())));
        drop(app.update(Message::AddressChanged("Seoul".into())));
        assert_eq!(
            (&app.name_input, &app.phone_input, &app.email_input, &app.address_input),
            (&"Alice".into(), &"010".into(), &"a@b.com".into(), &"Seoul".into())
        );
    }

    #[test]
    fn edit_cancel_and_update_without_selection_manage_form_state() {
        let mut app = app();
        drop(app.update(Message::EditAddress(address(Some(7)))));
        assert_eq!(app.editing_id, Some(7));
        assert_eq!(app.name_input, "Alice");

        drop(app.update(Message::CancelEdit));
        assert_eq!(app.editing_id, None);
        assert!(app.name_input.is_empty());

        drop(app.update(Message::UpdateAddress));
        assert_eq!(app.editing_id, None);
    }

    #[test]
    fn create_and_selected_update_clear_inputs_immediately() {
        let mut app = app();
        app.name_input = "Alice".into();
        app.phone_input = "010".into();
        app.email_input = "a@b.com".into();
        app.address_input = "Seoul".into();
        drop(app.update(Message::CreateAddress));
        assert!(app.name_input.is_empty());

        app.editing_id = Some(3);
        app.name_input = "Bob".into();
        drop(app.update(Message::UpdateAddress));
        assert_eq!(app.editing_id, None);
        assert!(app.name_input.is_empty());
    }

    #[test]
    fn loaded_result_replaces_data_or_records_error() {
        let mut app = app();
        drop(app.update(Message::AddressesLoaded(Ok(vec![address(Some(1))]))));
        assert_eq!(app.addresses.len(), 1);
        assert_eq!(app.error_message, None);

        let error = AppError::Validation(ValidationError::EmptyName);
        drop(app.update(Message::AddressesLoaded(Err(error))));
        assert_eq!(app.error_message, Some(AppError::Validation(ValidationError::EmptyName)));
        assert_eq!(app.addresses.len(), 1);
    }

    #[test]
    fn preferences_change_without_losing_contact_edit_or_error() {
        use crate::{i18n::Language,
                    theme::ThemeMode};
        let mut app = app();
        let contact = address(Some(7));
        app.addresses = vec![contact.clone()];
        drop(app.update(Message::EditAddress(contact.clone())));
        app.error_message = Some(AppError::Validation(ValidationError::EmptyName));

        for language in Language::ALL {
            drop(app.update(Message::LanguageSelected(language)));
            assert_eq!(app.title(), language.strings().window_title);
            for mode in ThemeMode::ALL {
                drop(app.update(Message::ThemeSelected(mode)));
                assert_eq!(app.theme(), mode.to_theme());
                assert_eq!(app.language, language);
                assert_eq!(app.editing_id, Some(7));
                assert_eq!(app.name_input, contact.name);
                assert_eq!(app.phone_input, contact.phone);
                assert_eq!(app.email_input, contact.email);
                assert_eq!(app.address_input, contact.address);
                assert_eq!(app.addresses, vec![contact.clone()]);
                assert_eq!(app.error_message, Some(AppError::Validation(ValidationError::EmptyName)));
                drop(app.view());
            }
        }
    }

    #[test]
    fn task_producing_messages_and_subscription_do_not_panic() {
        let mut app = app();
        drop(app.update(Message::LoadAddresses));
        drop(app.update(Message::DeleteAddress(1)));
        drop(app.update(Message::TabPressed {
            shift: false,
        }));
        drop(app.update(Message::TabPressed {
            shift: true,
        }));
        drop(app.subscription());
    }
}
