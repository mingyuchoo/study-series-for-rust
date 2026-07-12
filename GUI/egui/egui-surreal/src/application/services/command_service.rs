use crate::application::use_cases::{AuthUseCases,
                                    PersonUseCases,
                                    QueryUseCases};
use anyhow::Result;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Command {
    CreatePerson(String),
    DeletePerson(String),
    ListPeople,
    RawQuery(String),
    SignUp,
    SignIn(String, String),
    SignInRoot,
    Session,
}

pub struct CommandService {
    person_use_cases: Arc<PersonUseCases>,
    auth_use_cases: Arc<AuthUseCases>,
    query_use_cases: Arc<QueryUseCases>,
}

impl CommandService {
    pub fn new(person_use_cases: Arc<PersonUseCases>, auth_use_cases: Arc<AuthUseCases>, query_use_cases: Arc<QueryUseCases>) -> Self {
        Self {
            person_use_cases,
            auth_use_cases,
            query_use_cases,
        }
    }

    pub async fn handle_command(&self, command: Command) -> Result<String> {
        match command {
            | Command::CreatePerson(name) => self.person_use_cases.create_person(name).await,
            | Command::DeletePerson(id) => self.person_use_cases.delete_person(id).await,
            | Command::ListPeople => self.person_use_cases.list_people().await,
            | Command::SignUp => self.auth_use_cases.sign_up().await,
            | Command::SignIn(username, password) => self.auth_use_cases.sign_in(username, password).await,
            | Command::SignInRoot => self.auth_use_cases.sign_in_root().await,
            | Command::Session => self.auth_use_cases.get_session().await,
            | Command::RawQuery(query) => self.query_use_cases.execute_query(query).await,
        }
    }
}
