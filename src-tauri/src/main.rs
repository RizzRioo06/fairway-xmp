#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    fairway_core::desktop::run();
}
