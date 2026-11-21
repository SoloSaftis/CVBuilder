//! Desktop integration for the Linux AppImage.
//!
//! An AppImage started straight from a folder installs nothing, so GNOME finds no
//! `cvbuilder.desktop` for the window's app id and shows a generic icon in the dock. On each start
//! from an AppImage (only when `$APPIMAGE` is set, never in Flatpak or Snap, never for the tar.gz)
//! CVBuilder writes its own desktop entry and icon to the user's data folder:
//!
//! - `$XDG_DATA_HOME/applications/cvbuilder.desktop` (default `~/.local/share/...`), with `Exec=`
//!   and `TryExec=` pointing at `$APPIMAGE` and `StartupWMClass=cvbuilder`;
//! - `$XDG_DATA_HOME/icons/hicolor/256x256/apps/cvbuilder.png`.
//!
//! When the AppImage has moved, the next start rewrites the entry for the new path. Until then
//! `TryExec=` names a file that is gone, so desktops hide the stale entry. An entry CVBuilder did
//! not write (no `X-CVBuilder-AppImage` line, for example one the user put there by hand for the
//! tar.gz) is left alone. The text is built by pure functions so it can be tested anywhere; only
//! [`integrate`] touches the disk, and only Linux calls it.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::path::{Path, PathBuf};

/// The line that marks an entry as CVBuilder's own, so it may be rewritten.
const MARKER: &str = "X-CVBuilder-AppImage=true";
const ICON: &[u8] = include_bytes!("../../../assets/cvbuilder-256.png");

/// The environment variables that decide whether to integrate, read once so tests can pass their own.
#[derive(Default)]
pub(crate) struct Env {
    pub appimage: Option<String>,
    pub sandboxed: bool,
    pub data_home: Option<String>,
    pub home: Option<PathBuf>,
}

impl Env {
    fn current() -> Env {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        Env {
            appimage: var("APPIMAGE"),
            sandboxed: var("FLATPAK_ID").is_some() || var("SNAP").is_some() || Path::new("/.flatpak-info").exists(),
            data_home: var("XDG_DATA_HOME"),
            home: dirs::home_dir(),
        }
    }
}

/// Where the entry and icon go, and the AppImage they point at.
#[derive(Debug, PartialEq)]
pub(crate) struct Plan {
    pub appimage: String,
    pub desktop_file: PathBuf,
    pub icon_file: PathBuf,
}

/// Decides whether to integrate: only for an AppImage with an absolute, single-line path, outside
/// a sandbox, with a data folder to write to.
pub(crate) fn plan(env: &Env) -> Option<Plan> {
    if env.sandboxed {
        return None;
    }
    let appimage = env.appimage.as_ref()?;
    if !appimage.starts_with('/') || appimage.contains(['\n', '\r']) {
        return None;
    }
    // XDG: a relative XDG_DATA_HOME is invalid and must be ignored.
    let data = match &env.data_home {
        Some(d) if d.starts_with('/') => PathBuf::from(d),
        _ => env.home.as_ref()?.join(".local/share"),
    };
    Some(Plan {
        appimage: appimage.clone(),
        desktop_file: data.join("applications/cvbuilder.desktop"),
        icon_file: data.join("icons/hicolor/256x256/apps/cvbuilder.png"),
    })
}

/// Escapes a desktop entry string value (spec: `\\`, `\n`, `\t`, `\r`, `\s` only matter here).
fn escape_value(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\t', "\\t")
}

/// The program path as an `Exec=` argument: always quoted, with `"`, `` ` ``, `$` and `\`
/// escaped inside the quotes and `%` doubled (field codes), then escaped as a string value.
fn exec_arg(path: &str) -> String {
    let mut quoted = String::from("\"");
    for c in path.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(c);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(c),
        }
    }
    quoted.push('"');
    escape_value(&quoted)
}

/// The desktop entry for an AppImage at `appimage`.
pub(crate) fn desktop_entry(appimage: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=CVBuilder\n\
         Comment=CV-er som rekrutterere og søknadssystemer leser riktig\n\
         Comment[en]=CVs that recruiters and application systems read correctly\n\
         Exec={exec}\n\
         TryExec={try_exec}\n\
         Icon=cvbuilder\n\
         Terminal=false\n\
         Categories=Office;\n\
         StartupWMClass=cvbuilder\n\
         {MARKER}\n",
        exec = exec_arg(appimage),
        try_exec = escape_value(appimage),
    )
}

/// What to do with the entry already on disk (`None` when there is none).
#[derive(Debug, PartialEq)]
pub(crate) enum Action {
    /// Already right: nothing to write.
    Keep,
    /// Missing, or ours but stale (the AppImage moved, or the text changed): write it.
    Write,
    /// Someone else's entry: leave it alone.
    NotOurs,
}

pub(crate) fn action(existing: Option<&str>, wanted: &str) -> Action {
    match existing {
        None => Action::Write,
        Some(e) if e == wanted => Action::Keep,
        Some(e) if e.lines().any(|l| l.trim() == MARKER) => Action::Write,
        Some(_) => Action::NotOurs,
    }
}

/// Writes `bytes` to `path` through a temporary file next to it, so a half-written entry is never
/// read.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp-cvbuilder");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Applies `plan`: writes the icon if it differs and the entry if [`action`] says so.
pub(crate) fn apply(plan: &Plan) -> std::io::Result<Action> {
    let wanted = desktop_entry(&plan.appimage);
    let existing = std::fs::read_to_string(&plan.desktop_file).ok();
    let action = action(existing.as_deref(), &wanted);
    if action == Action::NotOurs {
        return Ok(action);
    }
    if std::fs::read(&plan.icon_file).ok().as_deref() != Some(ICON) {
        write_atomic(&plan.icon_file, ICON)?;
    }
    if action == Action::Write {
        write_atomic(&plan.desktop_file, wanted.as_bytes())?;
    }
    Ok(action)
}

/// Run at start on Linux. Failing is harmless (the dock just shows the generic icon), so errors
/// are only logged.
pub(crate) fn integrate() {
    if let Some(plan) = plan(&Env::current())
        && let Err(e) = apply(&plan)
    {
        eprintln!("CVBuilder: could not add the desktop entry for the AppImage: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(appimage: Option<&str>) -> Env {
        Env { appimage: appimage.map(String::from), home: Some(PathBuf::from("/home/kari")), ..Default::default() }
    }

    #[test]
    fn integrates_only_for_an_appimage_outside_a_sandbox() {
        assert_eq!(plan(&env(None)), None, "tar.gz or cargo run: no $APPIMAGE");
        assert_eq!(plan(&Env { sandboxed: true, ..env(Some("/home/kari/CVBuilder.AppImage")) }), None);
        assert_eq!(plan(&env(Some("CVBuilder.AppImage"))), None, "relative path");
        assert_eq!(plan(&env(Some("/home/kari/a\nb.AppImage"))), None, "newline in path");
        assert_eq!(plan(&Env { home: None, ..env(Some("/x.AppImage")) }), None);
        let p = plan(&env(Some("/home/kari/Apps/CVBuilder-1.3.1-x86_64.AppImage"))).unwrap();
        assert_eq!(p.appimage, "/home/kari/Apps/CVBuilder-1.3.1-x86_64.AppImage");
        assert_eq!(p.desktop_file, PathBuf::from("/home/kari/.local/share/applications/cvbuilder.desktop"));
        assert_eq!(p.icon_file, PathBuf::from("/home/kari/.local/share/icons/hicolor/256x256/apps/cvbuilder.png"));
    }

    #[test]
    fn follows_xdg_data_home_when_absolute() {
        let p = plan(&Env { data_home: Some("/data".into()), ..env(Some("/a.AppImage")) }).unwrap();
        assert_eq!(p.desktop_file, PathBuf::from("/data/applications/cvbuilder.desktop"));
        let p = plan(&Env { data_home: Some("rel".into()), ..env(Some("/a.AppImage")) }).unwrap();
        assert_eq!(p.desktop_file, PathBuf::from("/home/kari/.local/share/applications/cvbuilder.desktop"));
    }

    #[test]
    fn entry_points_at_the_appimage_and_the_window_class() {
        let e = desktop_entry("/home/kari/Apps/CVBuilder.AppImage");
        assert!(e.starts_with("[Desktop Entry]\n"));
        let lines: Vec<&str> = e.lines().collect();
        for want in [
            "Type=Application",
            "Name=CVBuilder",
            "Exec=\"/home/kari/Apps/CVBuilder.AppImage\"",
            "TryExec=/home/kari/Apps/CVBuilder.AppImage",
            "Icon=cvbuilder",
            "StartupWMClass=cvbuilder",
            "Categories=Office;",
            MARKER,
        ] {
            assert!(lines.contains(&want), "missing {want:?} in\n{e}");
        }
        assert!(e.ends_with('\n'));
    }

    #[test]
    fn exec_quotes_and_escapes_awkward_paths() {
        // Spaces need the quotes; % is a field code; " ` $ \ are escaped inside the quotes, and
        // every backslash is then escaped again as a string value.
        assert_eq!(exec_arg("/home/kari/Mine apper/CV.AppImage"), "\"/home/kari/Mine apper/CV.AppImage\"");
        assert_eq!(exec_arg("/a/100%/b"), "\"/a/100%%/b\"");
        assert_eq!(exec_arg("/a/\"q\"/b"), "\"/a/\\\\\"q\\\\\"/b\"");
        assert_eq!(exec_arg("/a/$HOME/`x`"), "\"/a/\\\\$HOME/\\\\`x\\\\`\"");
        assert_eq!(exec_arg("/a\\b"), "\"/a\\\\\\\\b\"");
        assert_eq!(escape_value("/a\\b\tc"), "/a\\\\b\\tc");
    }

    #[test]
    fn rewrites_only_its_own_stale_entry() {
        let wanted = desktop_entry("/new/CVBuilder.AppImage");
        assert_eq!(action(None, &wanted), Action::Write);
        assert_eq!(action(Some(&wanted), &wanted), Action::Keep);
        let moved = desktop_entry("/old/CVBuilder.AppImage");
        assert_eq!(action(Some(&moved), &wanted), Action::Write, "the AppImage moved");
        let by_hand = "[Desktop Entry]\nType=Application\nName=CVBuilder\nExec=/opt/cvbuilder/CVBuilder\n";
        assert_eq!(action(Some(by_hand), &wanted), Action::NotOurs);
    }

    #[test]
    fn apply_writes_icon_and_entry_then_keeps_them() {
        let dir = std::env::temp_dir().join(format!("cvbuilder-appimage-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let plan = Plan {
            appimage: "/home/kari/CVBuilder.AppImage".into(),
            desktop_file: dir.join("applications/cvbuilder.desktop"),
            icon_file: dir.join("icons/hicolor/256x256/apps/cvbuilder.png"),
        };
        assert_eq!(apply(&plan).unwrap(), Action::Write);
        assert_eq!(std::fs::read_to_string(&plan.desktop_file).unwrap(), desktop_entry(&plan.appimage));
        assert_eq!(std::fs::read(&plan.icon_file).unwrap(), ICON);
        assert_eq!(apply(&plan).unwrap(), Action::Keep);
        // Someone else's entry stays as it is.
        std::fs::write(&plan.desktop_file, "[Desktop Entry]\nName=Mine\n").unwrap();
        assert_eq!(apply(&plan).unwrap(), Action::NotOurs);
        assert_eq!(std::fs::read_to_string(&plan.desktop_file).unwrap(), "[Desktop Entry]\nName=Mine\n");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
