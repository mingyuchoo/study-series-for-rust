use application::usecases::AddressUseCases;
use domain::entities::Address;
use iced::{Element,
           Length,
           Subscription,
           Task,
           keyboard::{self,
                      key},
           widget::{Column,
                    button,
                    column,
                    container,
                    operation::{focus_next,
                                focus_previous},
                    row,
                    scrollable,
                    text,
                    text_input}};
use infrastructure::database::SqliteAddressRepository;
use std::sync::Arc;

const NOTO_SANS_KR: &[u8] = include_bytes!("../fonts/NotoSansKR-Regular.ttf");

#[derive(Debug, Clone)]
enum Message {
    NameChanged(String),
    PhoneChanged(String),
    EmailChanged(String),
    AddressChanged(String),
    CreateAddress,
    DeleteAddress(i64),
    EditAddress(Address),
    UpdateAddress,
    CancelEdit,
    LoadAddresses,
    AddressesLoaded(Result<Vec<Address>, String>),
    TabPressed { shift: bool },
}

struct AddressBook {
    usecases: Arc<AddressUseCases>,
    addresses: Vec<Address>,
    name_input: String,
    phone_input: String,
    email_input: String,
    address_input: String,
    editing_id: Option<i64>,
}

impl AddressBook {
    fn new() -> (Self, Task<Message>) {
        let repository = Arc::new(SqliteAddressRepository::new("addresses.db").expect("Failed to initialize database"));
        let usecases = Arc::new(AddressUseCases::new(repository));

        (
            Self {
                usecases,
                addresses: Vec::new(),
                name_input: String::new(),
                phone_input: String::new(),
                email_input: String::new(),
                address_input: String::new(),
                editing_id: None,
            },
            Task::done(Message::LoadAddresses),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
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

                self.name_input.clear();
                self.phone_input.clear();
                self.email_input.clear();
                self.address_input.clear();

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

                    self.name_input.clear();
                    self.phone_input.clear();
                    self.email_input.clear();
                    self.address_input.clear();
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
                self.name_input.clear();
                self.phone_input.clear();
                self.email_input.clear();
                self.address_input.clear();
                Task::none()
            },
            | Message::LoadAddresses => {
                let usecases = self.usecases.clone();
                Task::perform(async move { usecases.get_all_addresses() }, Message::AddressesLoaded)
            },
            | Message::AddressesLoaded(result) => {
                match result {
                    | Ok(addresses) => self.addresses = addresses,
                    | Err(_) => {},
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

    fn subscription(&self) -> Subscription<Message> {
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

    fn view(&self) -> Element<'_, Message> {
        let input_form = column![
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
        .padding(20);

        let action_buttons = if self.editing_id.is_some() {
            row![
                button("Update").on_press(Message::UpdateAddress).padding(10),
                button("Cancel").on_press(Message::CancelEdit).padding(10),
            ]
            .spacing(10)
        } else {
            row![button("Add").on_press(Message::CreateAddress).padding(10)]
        };

        let address_list: Element<_> = self
            .addresses
            .iter()
            .fold(Column::new().spacing(10), |col, addr| {
                col.push(
                    container(
                        column![
                            text(format!("Name: {}", addr.name)).size(18),
                            text(format!("Phone: {}", addr.phone)).size(14),
                            text(format!("Email: {}", addr.email)).size(14),
                            text(format!("Address: {}", addr.address)).size(14),
                            row![
                                button("Edit").on_press(Message::EditAddress(addr.clone())).padding(5),
                                button("Delete").on_press(Message::DeleteAddress(addr.id.unwrap())).padding(5),
                            ]
                            .spacing(10),
                        ]
                        .spacing(5)
                        .padding(10),
                    )
                    .padding(10)
                    .style(container::rounded_box),
                )
            })
            .into();

        let content = column![
            text("Address Book").size(32),
            input_form,
            action_buttons,
            text("Saved Addresses").size(24),
            scrollable(address_list).height(Length::Fill),
        ]
        .spacing(20)
        .padding(20);

        container(content).width(Length::Fill).height(Length::Fill).into()
    }
}

fn main() -> iced::Result {
    iced::application(AddressBook::new, AddressBook::update, AddressBook::view)
        .subscription(AddressBook::subscription)
        .title("Address Book")
        .font(NOTO_SANS_KR)
        .run()
}
