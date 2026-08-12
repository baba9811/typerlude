pub mod app;
pub mod cli;
pub mod config;
pub mod content;
mod diagnostic;
pub mod i18n;
pub mod model;
pub mod practice;
pub mod stats;
pub mod storage;
pub mod terminal;
pub mod theme;
pub mod typing;
pub mod ui;
pub mod update;
mod user_error;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
