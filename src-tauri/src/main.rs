// Sem console no build de release: o app vive na bandeja.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    nika_lib::run();
}
