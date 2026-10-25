#[cfg(any(target_os = "macos", windows, all(feature = "cli", unix)))]
mod command_output;
pub mod config;
pub mod data_browser;
pub mod database;
pub mod diagnostics;
mod durability;
pub mod hosts;
pub mod integration;
pub mod logs;
pub mod mail;
#[cfg(feature = "desktop")]
pub mod mail_images;
pub mod manager;
#[cfg(any(target_os = "macos", windows))]
pub mod node_tools;
pub mod php_settings;
pub mod process;
#[cfg(any(all(target_os = "macos", feature = "terminal"), windows))]
mod shell_environment;
pub mod tls;
pub mod updates;

#[cfg(feature = "terminal")]
pub mod terminal;

#[cfg(all(feature = "cli", unix))]
pub mod cli;

#[cfg(any(feature = "desktop", test))]
mod lifecycle;

#[cfg(feature = "desktop")]
mod startup;

#[cfg(feature = "desktop")]
mod tray;

#[cfg(feature = "desktop")]
pub mod desktop;
