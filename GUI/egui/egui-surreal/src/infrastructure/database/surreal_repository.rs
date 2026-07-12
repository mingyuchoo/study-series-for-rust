use crate::domain::{AuthParams,
                    AuthRepository,
                    Person,
                    PersonData,
                    PersonRepository,
                    QueryRepository};
use anyhow::{Error,
             Result};
use async_trait::async_trait;
use faker_rand::en_us::names::FirstName;
use serde::{Deserialize,
            Serialize};
use std::ops::Deref;
use surrealdb::{RecordIdKey,
                Surreal,
                engine::remote::ws::{Client,
                                     Ws},
                opt::auth::{Record,
                            Root}};

const PERSON: &str = "person";

#[derive(Serialize, Deserialize)]
struct Params<'a> {
    name: &'a str,
    pass: &'a str,
}

pub struct SurrealRepository {
    client: Surreal<Client>,
}

impl Deref for SurrealRepository {
    type Target = Surreal<Client>;

    fn deref(&self) -> &Self::Target { &self.client }
}

impl SurrealRepository {
    pub async fn new(url: &str) -> Result<Self> {
        let client = Surreal::new::<Ws>(url).await?;

        client
            .signin(Root {
                username: "root",
                password: "root",
            })
            .await?;

        client.use_ns("test").use_db("test").await?;

        // Initialize schema
        client
            .query(
                "DEFINE TABLE person SCHEMALESS
                PERMISSIONS FOR
                    CREATE, SELECT WHERE $auth,
                    FOR UPDATE, DELETE WHERE created_by = $auth;
            DEFINE FIELD name ON TABLE person TYPE string;
            DEFINE FIELD created_by ON TABLE person VALUE $auth READONLY;

            DEFINE INDEX unique_name ON TABLE user FIELDS name UNIQUE;
            DEFINE ACCESS account ON DATABASE TYPE RECORD
            SIGNUP ( CREATE user SET name = $name, pass = crypto::argon2::generate($pass) )
            SIGNIN ( SELECT * FROM user WHERE name = $name AND crypto::argon2::compare(pass, $pass) )
            DURATION FOR TOKEN 15m, FOR SESSION 12h
            ;",
            )
            .await?;

        Ok(Self {
            client,
        })
    }
}

trait StringIt {
    fn string(self) -> Result<String, Error>;
}

impl StringIt for Option<Person> {
    fn string(self) -> Result<String, Error> {
        match self {
            | Some(t) => Ok(format!("{t:?}")),
            | None => Ok("[]".into()),
        }
    }
}

#[async_trait]
impl PersonRepository for SurrealRepository {
    async fn create_person(&self, person_data: PersonData) -> Result<Option<Person>> { Ok(self.create::<Option<Person>>(PERSON).content(person_data).await?) }

    async fn delete_person(&self, id: Option<String>) -> Result<String> {
        match id {
            | None => {
                let res: Vec<Person> = self.delete(PERSON).await?;
                Ok(format!("{res:?}"))
            },
            | Some(id_str) => {
                let key = RecordIdKey::from(id_str);
                self.delete::<Option<Person>>((PERSON, key)).await?.string()
            },
        }
    }

    async fn list_people(&self) -> Result<Vec<Person>> { Ok(self.select(PERSON).await?) }
}

#[async_trait]
impl AuthRepository for SurrealRepository {
    async fn sign_up(&self) -> Result<String> {
        let name = rand::random::<FirstName>().to_string();
        let pass = rand::random::<FirstName>().to_string();

        self.signup(Record {
            access: "account",
            namespace: "test",
            database: "test",
            params: Params {
                name: &name,
                pass: &pass,
            },
        })
        .await?;

        Ok(format!("New user created!\n\n{{ \"name\": \"{name}\", \n \"pass\": \"{pass}\" }}"))
    }

    async fn sign_in(&self, params: AuthParams) -> Result<String> {
        self.signin(Record {
            access: "account",
            namespace: "test",
            database: "test",
            params: Params {
                name: &params.name,
                pass: &params.pass,
            },
        })
        .await?;

        Ok(format!("Signed in as {}!", params.name))
    }

    async fn sign_in_root(&self) -> Result<String> {
        self.signin(Root {
            username: "root",
            password: "root",
        })
        .await?;

        Ok("Back to root!".to_string())
    }

    async fn get_session(&self) -> Result<String> {
        Ok(self
            .query("RETURN <string>$session")
            .await?
            .take::<Option<String>>(0)?
            .unwrap_or("No session data found!".into()))
    }
}

#[async_trait]
impl QueryRepository for SurrealRepository {
    async fn execute_raw_query(&self, query: String) -> Result<String> {
        match self.query(query).await {
            | Ok(ok) => Ok(format!("{ok:?}")),
            | Err(e) => Ok(e.to_string()),
        }
    }
}
