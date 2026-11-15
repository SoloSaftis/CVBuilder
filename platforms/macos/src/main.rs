//! CVBuilder for macOS. Everything except packaging (the .app bundle) lives in crates/.
fn main() -> cvbuilder_app::Result {
    cvbuilder_app::run()
}
