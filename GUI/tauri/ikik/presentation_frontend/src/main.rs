mod app;
mod components;
mod i18n;
mod mode;
mod models;
mod services;
mod store;
mod theme;

use app::App;
use dioxus::prelude::*;
use dioxus_logger::tracing::Level;

fn main() {
    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    launch(App);
}
