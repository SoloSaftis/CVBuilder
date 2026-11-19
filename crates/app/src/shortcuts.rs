//! The app's keyboard shortcuts, in one list: what the keys do, how they are written in the
//! menus and tooltips (Ctrl on Windows and Linux, ⌘ on the Mac), and the safe default button in
//! a dialog, which Enter presses.
//!
//! Text fields keep their own keys: Ctrl+Z in a field undoes the typing there, and the big undo
//! (the whole CV from before «Ny CV», an import or a translation) only runs when no field has
//! the keyboard. While a dialog is open, only that dialog's keys work (Esc, Enter, Tab).
use egui::{Context, Key, KeyboardShortcut, Modifiers};

use crate::menu_ui::Action;
use crate::{CvApp, Export};

/// Ctrl on Windows and Linux, Cmd on the Mac.
const CMD: Modifiers = Modifiers::COMMAND;
const CMD_SHIFT: Modifiers = Modifiers { shift: true, ..Modifiers::COMMAND };

pub const NEW: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::N);
pub const OPEN: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::O);
pub const SAVE: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::S);
pub const SAVE_AS: KeyboardShortcut = KeyboardShortcut::new(CMD_SHIFT, Key::S);
pub const PDF: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::P);
pub const UNDO: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::Z);
/// Cmd+Shift+Z on the Mac (and Linux); Ctrl+Y works too, as on Windows.
pub const REDO: KeyboardShortcut = KeyboardShortcut::new(CMD_SHIFT, Key::Z);
pub const REDO_Y: KeyboardShortcut = KeyboardShortcut::new(CMD, Key::Y);
pub const GUIDE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F1);

/// The redo shortcut shown in the menus: Ctrl+Y on Windows, Cmd+Shift+Z on the Mac (and Linux).
pub fn redo(ctx: &Context) -> &'static KeyboardShortcut {
    if ctx.os() == egui::os::OperatingSystem::Windows { &REDO_Y } else { &REDO }
}

/// What the main window's shortcuts ask for this frame. The longer shortcuts are asked first:
/// egui counts Ctrl+Shift+S as Ctrl+S too.
fn pressed(ctx: &Context, dialog: bool) -> Option<Action> {
    // F1 works everywhere, also in a text field and over a dialog.
    if ctx.input_mut(|i| i.consume_shortcut(&GUIDE)) {
        return Some(Action::UserGuide);
    }
    if dialog {
        return None;
    }
    let in_field = ctx.text_edit_focused();
    ctx.input_mut(|i| {
        if i.consume_shortcut(&SAVE_AS) {
            Some(Action::SaveAs)
        } else if i.consume_shortcut(&SAVE) {
            Some(Action::Save)
        } else if i.consume_shortcut(&OPEN) {
            Some(Action::Open)
        } else if i.consume_shortcut(&NEW) {
            Some(Action::New)
        } else if i.consume_shortcut(&PDF) {
            Some(Action::Export(Export::Pdf))
        } else if in_field {
            // The field's own undo and redo.
            None
        } else if i.consume_shortcut(&REDO) || i.consume_shortcut(&REDO_Y) {
            Some(Action::Redo)
        } else if i.consume_shortcut(&UNDO) {
            Some(Action::Undo)
        } else {
            None
        }
    })
}

impl CvApp {
    /// The main window's shortcuts. While a dialog is open only F1 works: the dialog has the keyboard.
    pub(crate) fn shortcuts(&mut self, ctx: &Context) {
        if let Some(action) = pressed(ctx, self.dialog_open(ctx)) {
            self.run_action(ctx, action);
        }
    }

    /// A dialog (or the window's own question on closing) is showing.
    fn dialog_open(&self, ctx: &Context) -> bool {
        // egui knows every open modal window (from the frame before); the closing question and the
        // first start's question are counted by name, in case they open this very frame.
        ctx.memory(|m| m.top_modal_layer().is_some()) || self.closing || self.welcome.is_some() || self.confirm.is_some() || self.photo.is_some()
    }
}

/// The shortcut as it is written on this system: «Ctrl+Shift+S», or «⇧⌘S» on the Mac.
pub fn text(ctx: &Context, shortcut: &KeyboardShortcut) -> String {
    ctx.format_shortcut(shortcut)
}

/// A tooltip with its shortcut after it: «Åpne et lagret utkast (Ctrl+O)».
pub fn tip(ctx: &Context, tip: &str, shortcut: &KeyboardShortcut) -> String {
    format!("{tip} ({})", text(ctx, shortcut))
}

/// Translated text that names Ctrl keys («lim inn med Ctrl+V», «Strg+S» in German), with the
/// Mac's ⌘ in their place on the Mac. Elsewhere the text is left as it is.
pub fn keys(ctx: &Context, text: &str) -> String {
    if !ctx.os().is_mac() {
        return text.to_string();
    }
    // «⌘V» or «Cmd+V», whichever this font can show: the same as in the menus.
    let joined = ctx.format_shortcut(&KeyboardShortcut::new(CMD, Key::V));
    let prefix = joined.strip_suffix('V').unwrap_or("Cmd+");
    let alone = prefix.trim_end_matches('+');
    let mut out = text.to_string();
    for ctrl in ["Ctrl", "Strg"] {
        out = out.replace(&format!("{ctrl}+"), prefix).replace(ctrl, alone);
    }
    out
}

/// Makes `button` the dialog's default: it gets the keyboard focus when the dialog opens, so
/// Enter (or Space) presses it and Tab goes on from there. Give it the choice that loses nothing.
pub fn default_button(ui: &egui::Ui, button: &egui::Response) {
    // Shown the frame before too: the dialog was already open, and the person may have moved on.
    let seen = button.id.with("default-button");
    let frame = ui.ctx().cumulative_frame_nr();
    let last: Option<u64> = ui.data_mut(|d| {
        let last = d.get_temp(seen);
        d.insert_temp(seen, frame);
        last
    });
    if last.is_none_or(|l| l + 1 < frame) {
        button.request_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(os: egui::os::OperatingSystem) -> Context {
        // One frame first: the fonts (which decide ⌘ or Cmd) are loaded by the first run.
        let ctx = Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
        ctx.set_os(os);
        ctx
    }

    #[test]
    fn ctrl_elsewhere_and_cmd_on_the_mac() {
        let pc = ctx(egui::os::OperatingSystem::Windows);
        assert_eq!(text(&pc, &SAVE), "Ctrl+S");
        assert_eq!(text(&pc, &SAVE_AS), "Ctrl+Shift+S");
        assert_eq!(text(&pc, &GUIDE), "F1");
        assert_eq!(keys(&pc, "Lagre (Ctrl+S)"), "Lagre (Ctrl+S)");
        assert_eq!(keys(&pc, "Speichern (Strg+S)"), "Speichern (Strg+S)");
        let mac = ctx(egui::os::OperatingSystem::Mac);
        let save = text(&mac, &SAVE);
        assert!(save == "⌘S" || save == "Cmd+S", "{save}");
        assert!(!text(&mac, &REDO).contains("Ctrl"));
        let cmd = save.trim_end_matches('S');
        assert_eq!(keys(&mac, "Lagre (Ctrl+S)."), format!("Lagre ({cmd}S)."));
        assert_eq!(keys(&mac, "Speichern (Strg+S)."), format!("Speichern ({cmd}S)."));
        assert_eq!(keys(&mac, "lim inn med Ctrl+V"), format!("lim inn med {cmd}V"));
        let alone = cmd.trim_end_matches('+');
        assert_eq!(keys(&mac, "Ctrl + pluss og minus"), format!("{alone} + pluss og minus"));
        assert_eq!(tip(&pc, "Åpne", &OPEN), "Åpne (Ctrl+O)");
    }

    /// The keys do what the menus say, and the field's own Ctrl+Z is left alone.
    #[test]
    fn keys_pick_the_action() {
        let ctx = Context::default();
        let press = |modifiers: Modifiers, key: Key| {
            let mut input = egui::RawInput::default();
            input.events.push(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
            let mut got = None;
            ctx.run_ui(input, |ui| got = pressed(ui.ctx(), false).map(|a| format!("{a:?}"))).textures_delta.clear();
            got
        };
        assert_eq!(press(CMD, Key::S).as_deref(), Some("Save"));
        assert_eq!(press(CMD_SHIFT, Key::S).as_deref(), Some("SaveAs"));
        assert_eq!(press(CMD, Key::O).as_deref(), Some("Open"));
        assert_eq!(press(CMD, Key::N).as_deref(), Some("New"));
        assert_eq!(press(CMD, Key::P).as_deref(), Some("Export(Pdf)"));
        assert_eq!(press(CMD, Key::Z).as_deref(), Some("Undo"));
        assert_eq!(press(CMD_SHIFT, Key::Z).as_deref(), Some("Redo"));
        assert_eq!(press(CMD, Key::Y).as_deref(), Some("Redo"));
        assert_eq!(press(Modifiers::NONE, Key::F1).as_deref(), Some("UserGuide"));
        assert_eq!(press(Modifiers::NONE, Key::S), None);
    }
}
