//! CVBuilder for Windows. Everything except packaging and the exe's icon lives in ../shared.
// No console window behind the app in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> cvbuilder_app::Result {
    cvbuilder_app::run()
}
