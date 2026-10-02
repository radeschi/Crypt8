// Developed by: Maicon Radeschi
// Email: radeschi@me.com
// May the Force be with you!
// 2026

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    crypt8_lib::run()
}
