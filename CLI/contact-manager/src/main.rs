use std::{collections::HashMap,
          fs::{File,
               OpenOptions},
          io::{Read,
               Write},
          path::PathBuf};
use structopt::StructOpt;
use thiserror::Error;

#[derive(Debug)]
struct Record {
    id: i64,
    name: String,
    email: Option<String>,
}

struct Records {
    inner: HashMap<i64, Record>,
}

impl Records {
    fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    fn add(&mut self, record: Record) { self.inner.insert(record.id, record); }

    fn edit(&mut self, id: i64, name: &str, email: Option<String>) {
        self.inner.insert(
            id,
            Record {
                id,
                name: name.to_string(),
                email,
            },
        );
    }

    fn into_vec(mut self) -> Vec<Record> {
        let mut records: Vec<_> = self.inner.drain().map(|kv| kv.1).collect();

        records.sort_by_key(|record| record.id);

        records
    }

    fn next_id(&self) -> i64 {
        let mut ids: Vec<_> = self.inner.keys().collect();

        ids.sort();

        match ids.pop() {
            | Some(id) => id + 1,
            | None => 1,
        }
    }

    fn search(&self, name: &str) -> Vec<&Record> {
        self.inner
            .values()
            .filter(|record| record.name.to_lowercase().contains(&name.to_lowercase()))
            .collect()
    }

    fn remove(&mut self, id: i64) -> Option<Record> { self.inner.remove(&id) }
}

#[derive(Debug, Error)]
enum ParseError {
    #[error("id must be a number: {0}")]
    InvalidId(#[from] std::num::ParseIntError),

    #[error("empty record")]
    EmptyRecord,

    #[error("missing field: {0}")]
    MissingField(String),
}

#[derive(Debug, StructOpt)]
#[structopt(about = "project: contact manager")]
struct Opt {
    #[structopt(short, parse(from_os_str), default_value = "data.csv")]
    data_file: PathBuf,

    #[structopt(subcommand)]
    cmd: Command,

    #[structopt(short, help = "verbose")]
    verbose: bool,
}

#[derive(Debug, StructOpt)]
enum Command {
    Add {
        name: String,
        #[structopt(short)]
        email: Option<String>,
    },
    Edit {
        id: i64,
        name: String,
        #[structopt(short)]
        email: Option<String>,
    },
    List {},
    Remove {
        id: i64,
    },
    Search {
        query: String,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = Opt::from_args();
    if let Err(e) = run(opt) {
        println!("an error occurred: {}", e);
    }

    Ok(())
}

fn run(opt: Opt) -> Result<(), std::io::Error> {
    match opt.cmd {
        | Command::Add {
            name,
            email,
        } => {
            let mut records = load_records(opt.data_file.clone(), opt.verbose)?;
            let next_id = records.next_id();
            records.add(Record {
                id: next_id,
                name,
                email,
            });
            save_records(opt.data_file, records)?;
        },
        | Command::Edit {
            id,
            name,
            email,
        } => {
            let mut records = load_records(opt.data_file.clone(), opt.verbose)?;
            records.edit(id, &name, email);
            save_records(opt.data_file, records)?;
        },
        | Command::List {
            ..
        } => {
            let records = load_records(opt.data_file, opt.verbose)?;

            records.into_vec().iter().for_each(|record| println!("{:?}", record));
        },
        | Command::Remove {
            id,
        } => {
            let mut records = load_records(opt.data_file.clone(), opt.verbose)?;
            if records.remove(id).is_some() {
                save_records(opt.data_file, records)?;
                println!("record deleted");
            } else {
                println!("record not found");
            }
        },
        | Command::Search {
            query,
        } => {
            let records = load_records(opt.data_file, opt.verbose)?;
            let results = records.search(&query);
            if results.is_empty() {
                println!("no records found");
            } else {
                results.iter().for_each(|record| println!("{:?}", record));
            }
        },
    }
    Ok(())
}

fn load_records(file_name: PathBuf, verbose: bool) -> std::io::Result<Records> {
    let mut file = File::open(file_name)?;
    let mut buffer = String::new();

    file.read_to_string(&mut buffer)?;

    Ok(parse_records(buffer, verbose))
}

fn parse_records(records: String, verbose: bool) -> Records {
    let mut new_records = Records::new();

    for (num, record) in records.split('\n').enumerate() {
        if !record.is_empty() {
            match parse_record(record) {
                | Ok(record) => new_records.add(record),
                | Err(e) =>
                    if verbose {
                        println!("error on line number {}: {}\n > \"{}\"\n", num + 1, e, record);
                    },
            }
        }
    }
    new_records
}

fn parse_record(record: &str) -> Result<Record, ParseError> {
    let fields: Vec<&str> = record.split(',').collect();

    let id = match fields.first() {
        | Some(id) => id.parse::<i64>()?,
        | None => return Err(ParseError::EmptyRecord),
    };

    let name = match fields.get(1).filter(|name| !name.is_empty()) {
        | Some(name) => name.to_string(),
        | None => return Err(ParseError::MissingField("name".to_owned())),
    };

    let email = fields.get(2).map(|email| email.to_string()).filter(|email| !email.is_empty());

    Ok(Record {
        id,
        name,
        email,
    })
}

fn save_records(file_name: PathBuf, records: Records) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).truncate(true).open(file_name)?;

    file.write_all(b"id,name,email\n")?;

    for record in records.into_vec().into_iter() {
        let email = match record.email {
            | Some(email) => email,
            | None => "".to_owned(),
        };

        let line = format!("{},{},{}\n", record.id, record.name, email);
        file.write_all(line.as_bytes())?;
    }

    file.flush()?;

    Ok(())
}
