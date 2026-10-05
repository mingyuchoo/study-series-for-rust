use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    #[arg(short, long, default_value_t = 8000)]
    pub port: u16,
}

impl Config {
    pub fn from_args() -> Self { Self::parse() }
}
