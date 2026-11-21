//! Hjelp → «Se etter oppdateringer»: asks GitHub for the newest release when the person clicks,
//! and says whether there is a newer one, with a link to its download page. Nothing is downloaded
//! or installed by itself. A copy from the Microsoft Store is kept up to date by the Store.
use std::sync::mpsc::{Receiver, channel};

use egui::RichText;

use crate::menu_ui::VERSION;
use crate::{CvApp, i18n, theme};

/// CVBuilder's page in the Microsoft Store app.
const STORE_PAGE: &str = "ms-windows-store://pdp/?ProductId=9NCKXG5ZTL7L";

pub(crate) enum UpdateCheck {
    Checking(Receiver<Result<cv_ai::updates::Release, String>>),
    Done(Result<cv_ai::updates::Release, String>),
}

impl UpdateCheck {
    /// Starts asking GitHub, on its own thread.
    pub(crate) fn start(ctx: &egui::Context) -> UpdateCheck {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(cv_ai::updates::latest());
            ctx.request_repaint();
        });
        UpdateCheck::Checking(rx)
    }
}

/// Whether this copy was installed from the Microsoft Store (an MSIX package lives under
/// «WindowsApps»), which updates it by itself.
fn from_store() -> bool {
    std::env::current_exe().is_ok_and(|p| p.to_string_lossy().to_lowercase().contains("\\windowsapps\\"))
}

impl CvApp {
    pub(crate) fn update_dialog(&mut self, ctx: &egui::Context) {
        let Some(check) = &mut self.update_check else {
            return;
        };
        if let UpdateCheck::Checking(rx) = check {
            if let Ok(answer) = rx.try_recv() {
                *check = UpdateCheck::Done(answer);
            }
        }
        let mut close = false;
        let mut again = false;
        let r = egui::Modal::new(egui::Id::unique("cvbuilder.updates")).show(ctx, |ui| {
            ui.set_max_width(440.0);
            ui.label(RichText::new(i18n::t("Se etter oppdateringer")).size(18.0).family(theme::bold()));
            ui.add_space(8.0);
            let mut link: Option<(String, &'static str)> = None;
            // The check failed: «Prøv igjen» is the main button, the download page comes second.
            let mut failed = false;
            match check {
                UpdateCheck::Checking(_) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(i18n::t("Ser etter oppdateringer …"));
                    });
                }
                UpdateCheck::Done(Err(e)) => {
                    let (text, technical) = cv_core::plain::split(e);
                    ui.label(i18n::tdyn(text));
                    if let Some(technical) = technical {
                        ui.add_space(4.0);
                        theme::hint(ui, &i18n::tf("Detaljer: {detail}", &[("detail", technical)]));
                    }
                    failed = true;
                    link = Some((cv_ai::updates::RELEASES_PAGE.to_string(), i18n::t("Åpne nedlastingssiden")));
                }
                UpdateCheck::Done(Ok(release)) if cv_ai::updates::is_newer(&release.version, VERSION) => {
                    ui.label(RichText::new(i18n::tf("CVBuilder {new} er ute. Du har {old}.", &[("new", &release.version), ("old", VERSION)])).strong());
                    link = Some((release.page.clone(), i18n::t("Åpne nedlastingssiden")));
                }
                UpdateCheck::Done(Ok(_)) => {
                    ui.label(i18n::tf("Du har den nyeste versjonen ({version}).", &[("version", VERSION)]));
                }
            }
            if from_store() {
                ui.add_space(6.0);
                theme::hint(ui, i18n::t("Installert fra Microsoft Store, som oppdaterer CVBuilder av seg selv."));
                if link.is_some() {
                    link = Some((STORE_PAGE.to_string(), i18n::t("Åpne i Microsoft Store")));
                }
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if failed && theme::primary(ui, i18n::t("Prøv igjen"), i18n::t("Spør GitHub på nytt om den nyeste versjonen"), true).clicked() {
                    again = true;
                }
                if let Some((url, label)) = &link {
                    let tip = i18n::t("Åpner siden i nettleseren. Ingenting lastes ned av seg selv.");
                    let open = if failed { ui.button(*label).on_hover_text(tip) } else { theme::primary(ui, label, tip, true) };
                    if open.clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                    }
                }
                let default = ui.button(i18n::t("Lukk")).on_hover_text(i18n::t("Lukk vinduet (Esc)"));
                crate::shortcuts::default_button(ui, &default);
                if default.clicked() {
                    close = true;
                }
            });
        });
        if again {
            self.update_check = Some(UpdateCheck::start(ctx));
        } else if close || r.should_close() {
            self.update_check = None;
        }
    }
}
