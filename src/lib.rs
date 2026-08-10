pub mod app;
pub mod cli;
pub mod config;
pub mod content;
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

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
