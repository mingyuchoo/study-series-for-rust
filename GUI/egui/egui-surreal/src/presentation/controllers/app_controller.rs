use crate::{application::services::Command,
            domain::{AppMessage,
                     MessageType},
            presentation::state::AppState};
use std::sync::mpsc::{Receiver,
                      Sender};

pub struct AppController {
    pub state: AppState,
    command_sender: Sender<Command>,
    response_receiver: Receiver<String>,
}

impl AppController {
    pub fn new(command_sender: Sender<Command>, response_receiver: Receiver<String>) -> Self {
        Self {
            state: AppState::default(),
            command_sender,
            response_receiver,
        }
    }

    pub fn create_person(&mut self, name: String) { self.send_command(Command::CreatePerson(name)); }

    pub fn delete_person(&mut self, id: String) { self.send_command(Command::DeletePerson(id)); }

    pub fn list_people(&mut self) { self.send_command(Command::ListPeople); }

    pub fn sign_up(&mut self) { self.send_command(Command::SignUp); }

    pub fn sign_in(&mut self, username: String, password: String) { self.send_command(Command::SignIn(username, password)); }

    pub fn sign_in_root(&mut self) { self.send_command(Command::SignInRoot); }

    pub fn get_session(&mut self) { self.send_command(Command::Session); }

    pub fn execute_query(&mut self, query: String) { self.send_command(Command::RawQuery(query)); }

    fn send_command(&mut self, command: Command) {
        if let Err(e) = self.command_sender.send(command) {
            self.add_message(e.to_string(), MessageType::Error);
        } else {
            self.state.is_loading = true;
        }
    }

    pub fn add_message(&mut self, content: String, msg_type: MessageType) {
        self.state.messages.push(AppMessage {
            content,
            msg_type,
            timestamp: std::time::Instant::now(),
        });

        // Keep only last 10 messages
        if self.state.messages.len() > 10 {
            self.state.messages.remove(0);
        }
    }

    pub fn handle_response(&mut self) {
        if let Ok(response) = self.response_receiver.try_recv() {
            self.state.is_loading = false;

            // Determine message type based on response content
            let msg_type = if response.contains("Error") || response.contains("error") {
                MessageType::Error
            } else {
                MessageType::Success
            };

            // Route response to appropriate field
            match self.state.current_tab {
                | crate::presentation::ui::AppTab::People =>
                    if response.contains("Person") || response.contains("person") {
                        self.state.people_list = response.clone();
                    },
                | crate::presentation::ui::AppTab::Authentication =>
                    if response.contains("Signed in") {
                        self.state.current_user = response.clone();
                    },
                | crate::presentation::ui::AppTab::Query => {
                    self.state.query_result = response.clone();
                },
                | crate::presentation::ui::AppTab::Session => {
                    self.state.session_info = response.clone();
                },
            }

            self.add_message(response, msg_type);
        }
    }
}
