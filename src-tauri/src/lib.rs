pub mod config;
pub mod database;
pub mod integration;
pub mod manager;
pub mod process;

#[cfg(any(feature = "desktop", test))]
mod lifecycle;

#[cfg(feature = "desktop")]
mod tray;

#[cfg(feature = "desktop")]
pub mod desktop;
