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
                    | Err(error) => self.error_message = Some(error.to_string()),
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
