//! Opening web links in the person's browser: the user guide, «Report a problem», update pages
//! and every hyperlink in the window.
//!
//! egui only asks for a link to be opened (`OutputCommand::OpenUrl`). At the end of each frame
//! [`open_requested`] takes those requests out of the frame's output and opens them on a thread of
//! their own, so a slow or hanging opener never holds up the window. When no way of opening works,
//! a small window shows the link so it can be copied into a browser by hand.
//!
//! - Windows and macOS: the system's own opener, through the `webbrowser` crate.
//! - Linux: `xdg-open`, then `gio open`, then the desktop portal (`org.freedesktop.portal.OpenURI`,
//!   through `gdbus` or `busctl`). In a sandbox (Flatpak, Snap) the portal is tried first, since it
//!   is the only way out of it.
use std::sync::Mutex;

use crate::i18n;
use crate::theme;

/// The link that could not be opened, shown until the person closes the window.
static FAILED: Mutex<Option<String>> = Mutex::new(None);

/// Takes this frame's link requests and opens them, and shows the copy window after a failure.
/// Called once at the end of each frame.
pub fn open_requested(ctx: &egui::Context) {
    let mut urls = Vec::new();
    ctx.output_mut(|o| {
        o.commands.retain(|c| match c {
            egui::OutputCommand::OpenUrl(open) => {
                urls.push(open.url.clone());
                false
            }
            _ => true,
        });
    });
    for url in urls {
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new().name("open-link".into()).spawn({
            let url = url.clone();
            move || {
                if !open(&url) {
                    set_failed(Some(url));
                    ctx.request_repaint();
                }
            }
        });
        if spawned.is_err() {
            set_failed(Some(url));
        }
    }
    failed_dialog(ctx);
}

fn set_failed(url: Option<String>) {
    if let Ok(mut f) = FAILED.lock() {
        *f = url;
    }
}

fn failed_dialog(ctx: &egui::Context) {
    let Some(url) = FAILED.lock().ok().and_then(|f| f.clone()) else {
        return;
    };
    let mut close = false;
    let r = egui::Modal::new(egui::Id::unique("cvbuilder.open-link-failed")).show(ctx, |ui| {
        ui.set_max_width(520.0);
        ui.label(egui::RichText::new("CVBuilder").size(18.0).family(theme::bold()));
        ui.add_space(6.0);
        ui.add(egui::Label::new(egui::RichText::new(&url).monospace()).selectable(true).wrap());
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if theme::primary(ui, i18n::t("Kopier"), i18n::t("Kopier til utklippstavlen"), true).clicked() {
                ui.ctx().copy_text(url.clone());
            }
            let default = ui.button(i18n::t("Lukk")).on_hover_text(i18n::t("Lukk vinduet (Esc)"));
            crate::shortcuts::default_button(ui, &default);
            if default.clicked() {
                close = true;
            }
        });
    });
    if close || r.should_close() {
        set_failed(None);
    }
}

/// Only web and mail links are handed to the system, never a file or a program.
fn allowed(url: &str) -> bool {
    let lower = url.trim_start().to_ascii_lowercase();
    ["https://", "http://", "mailto:"].iter().any(|s| lower.starts_with(s))
}

/// Opens `url`; `false` when nothing could. May block until the opener answers.
fn open(url: &str) -> bool {
    allowed(url) && open_with_system(url)
}

#[cfg(not(target_os = "linux"))]
fn open_with_system(url: &str) -> bool {
    webbrowser::open(url).is_ok()
}

#[cfg(target_os = "linux")]
fn open_with_system(url: &str) -> bool {
    use std::process::{Command, Stdio};

    /// Runs one opener and waits for its answer (they hand the link over and return).
    fn run(program: &str, args: &[&str]) -> bool {
        Command::new(program).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
    }

    /// The desktop portal: works from inside Flatpak and Snap, and on any desktop that has one.
    fn portal(url: &str) -> bool {
        // gdbus reads its arguments as GVariant text: quote the link as a GVariant string.
        let quoted = format!("'{}'", url.replace('\\', "\\\\").replace('\'', "\\'"));
        run(
            "gdbus",
            &[
                "call",
                "--session",
                "--dest",
                "org.freedesktop.portal.Desktop",
                "--object-path",
                "/org/freedesktop/portal/desktop",
                "--method",
                "org.freedesktop.portal.OpenURI.OpenURI",
                "''",
                &quoted,
                "{}",
            ],
        ) || run("busctl", &["--user", "call", "org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop", "org.freedesktop.portal.OpenURI", "OpenURI", "ssa{sv}", "", url, "0"])
    }

    let sandboxed = std::env::var_os("FLATPAK_ID").is_some() || std::path::Path::new("/.flatpak-info").exists() || std::env::var_os("SNAP").is_some();
    if sandboxed && portal(url) {
        return true;
    }
    run("xdg-open", &[url]) || run("gio", &["open", url]) || (!sandboxed && portal(url))
}

#[cfg(test)]
mod tests {
    use super::allowed;

    #[test]
    fn only_web_and_mail_links_are_opened() {
        assert!(allowed("https://github.com/SoloSaftis/CVBuilder"));
        assert!(allowed("HTTP://example.com"));
        assert!(allowed("mailto:someone@example.com"));
        assert!(!allowed("file:///etc/passwd"));
        assert!(!allowed("--help"));
        assert!(!allowed("/usr/bin/true"));
    }
}
