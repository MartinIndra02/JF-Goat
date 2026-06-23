// Prevents additional console window on Windows in release, DO NOT REMOVE!!
// Modified to force rebuilding app icon resources.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    jfgoat_lib::run()
}
