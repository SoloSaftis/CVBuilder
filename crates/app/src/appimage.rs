//! Desktop integration for the Linux AppImage.
//!
//! An AppImage started straight from a folder installs nothing, so GNOME finds no
//! `cvbuilder.desktop` for the window's app id and shows a generic icon in the dock. On each start
//! from an AppImage (only when this program runs from the AppImage's `$APPDIR` and `$APPIMAGE` is
//! a file, never in Flatpak or Snap, never for the tar.gz) CVBuilder writes its own desktop entry
//! and icon to the user's data folder:
//!
//! - `$XDG_DATA_HOME/applications/cvbuilder.desktop` (default `~/.local/share/...`), with `Exec=`
//!   and `TryExec=` pointing at `$APPIMAGE` and `StartupWMClass=cvbuilder`;
//! - `$XDG_DATA_HOME/icons/hicolor/256x256/apps/cvbuilder.png`.
//!
//! When the AppImage has moved, the next start rewrites the entry for the new path. Until then
//! `TryExec=` names a file that is gone, so desktops hide the stale entry. An entry CVBuilder did
//! not write (no `X-CVBuilder-AppImage` line, for example one the user put there by hand for the
//! tar.gz), a symlink, a file that is not UTF-8 or one that can't be read is left alone. The text
//! is built by pure functions so it can be tested anywhere; only
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
    /// Whether `$APPIMAGE` names a regular file that exists.
    pub appimage_is_file: bool,
    /// `$APPDIR`, the mounted AppImage's folder, with symlinks resolved.
    pub appdir: Option<PathBuf>,
    /// This program, with symlinks resolved.
    pub exe: Option<PathBuf>,
    pub sandboxed: bool,
    pub data_home: Option<String>,
    pub home: Option<PathBuf>,
}

impl Env {
    fn current() -> Env {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let resolve = |p: PathBuf| std::fs::canonicalize(&p).unwrap_or(p);
        let appimage = var("APPIMAGE");
        Env {
            appimage_is_file: appimage.as_ref().is_some_and(|a| Path::new(a).is_file()),
            appimage,
            appdir: var("APPDIR").map(|d| resolve(PathBuf::from(d))),
            exe: std::env::current_exe().ok().map(resolve),
            sandboxed: var("FLATPAK_ID").is_some() || var("SNAP").is_some() || Path::new("/.flatpak-info").exists(),
            data_home: var("XDG_DATA_HOME"),
            home: dirs::home_dir(),
        }
    }
}

/// A Unix absolute path, checked as text so the tests mean the same on every system.
fn is_absolute(p: &Path) -> bool {
    p.to_str().is_some_and(|s| s.starts_with('/'))
}

/// Where the entry and icon go, and the AppImage they point at.
#[derive(Debug, PartialEq)]
pub(crate) struct Plan {
    pub appimage: String,
    pub desktop_file: PathBuf,
    pub icon_file: PathBuf,
}

/// Decides whether to integrate: only when this program runs from inside a mounted AppImage
/// (`$APPDIR` is set and the program lies under it), `$APPIMAGE` is an absolute, single-line path
/// to a file that exists, outside a sandbox, with an absolute data folder to write to.
///
/// Every program an AppImage starts inherits its `$APPDIR` and `$APPIMAGE`, so `$APPIMAGE` alone
/// may name some other program; requiring this program to lie under `$APPDIR` rules that out.
pub(crate) fn plan(env: &Env) -> Option<Plan> {
    if env.sandboxed || !env.appimage_is_file {
        return None;
    }
    let appimage = env.appimage.as_ref()?;
    if !appimage.starts_with('/') || appimage.contains(['\n', '\r']) {
        return None;
    }
    let (appdir, exe) = (env.appdir.as_deref()?, env.exe.as_deref()?);
    // `/` as APPDIR holds every program, so it proves nothing.
    if !is_absolute(appdir) || appdir.parent().is_none() || !exe.starts_with(appdir) {
        return None;
    }
    // XDG: a relative XDG_DATA_HOME is invalid and must be ignored; a relative HOME is refused.
    let data = match &env.data_home {
        Some(d) if d.starts_with('/') => PathBuf::from(d),
        _ => env.home.as_deref().filter(|h| is_absolute(h))?.join(".local/share"),
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

/// What is at `path` now: `None` when nothing is there, the text of a readable UTF-8 file, or
/// `NotOurs` for a symlink or a file that is not UTF-8 (CVBuilder writes neither). A file that
/// can't be read gives the read error, so it is never replaced.
fn read_existing(path: &Path) -> std::io::Result<Result<Option<String>, Action>> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Ok(None)),
        Err(e) => return Err(e),
        Ok(m) if m.file_type().is_symlink() => return Ok(Err(Action::NotOurs)),
        Ok(_) => {}
    }
    let bytes = std::fs::read(path)?;
    Ok(String::from_utf8(bytes).map(Some).map_err(|_| Action::NotOurs))
}

/// Writes `bytes` to `path` through a new temporary file next to it, so a half-written file is
/// never read. The temporary file has a name no one can guess ahead and is made with
/// `create_new`, so an existing file or symlink of that name is never opened; the rename then
/// replaces `path` itself, never a file a symlink there points at.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNT: AtomicU32 = AtomicU32::new(0);

    let dir = path.parent().ok_or_else(|| std::io::Error::other("no parent folder"))?;
    std::fs::create_dir_all(dir)?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("cvbuilder");
    let mut last = None;
    for _ in 0..8 {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        let tmp = dir.join(format!(".{name}.{}.{nanos:x}.{n}.tmp", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o644);
        let mut file = match options.open(&tmp) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                last = Some(e);
                continue;
            }
            Err(e) => return Err(e),
        };
        let written = file.write_all(bytes).and_then(|()| file.sync_all());
        drop(file);
        return written.and_then(|()| std::fs::rename(&tmp, path)).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        });
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("no free temporary file name")))
}

/// Applies `plan`: writes the icon if it differs and the entry if [`action`] says so. An entry
/// that can't be read is never replaced (the read error is returned); a symlink or a file that is
/// not UTF-8 counts as someone else's entry. A symlink in the icon's place is left alone too.
pub(crate) fn apply(plan: &Plan) -> std::io::Result<Action> {
    let wanted = desktop_entry(&plan.appimage);
    let action = match read_existing(&plan.desktop_file)? {
        Ok(existing) => action(existing.as_deref(), &wanted),
        Err(not_ours) => not_ours,
    };
    if action == Action::NotOurs {
        return Ok(action);
    }
    let icon_is_link = std::fs::symlink_metadata(&plan.icon_file).is_ok_and(|m| m.file_type().is_symlink());
    if !icon_is_link && std::fs::read(&plan.icon_file).ok().as_deref() != Some(ICON) {
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

    /// A CVBuilder AppImage mounted at `/tmp/.mount_CVBuiXy` and running from it.
    fn env(appimage: Option<&str>) -> Env {
        Env {
            appimage: appimage.map(String::from),
            appimage_is_file: appimage.is_some(),
            appdir: Some(PathBuf::from("/tmp/.mount_CVBuiXy")),
            exe: Some(PathBuf::from("/tmp/.mount_CVBuiXy/usr/bin/cvbuilder")),
            home: Some(PathBuf::from("/home/kari")),
            ..Default::default()
        }
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
    fn ignores_appimage_variables_inherited_from_another_program() {
        let other = Some("/home/kari/Apps/OtherTool.AppImage");
        // CVBuilder (tar.gz) started from another AppImage: its APPDIR and APPIMAGE are inherited,
        // but this program is not under that APPDIR.
        let inherited = Env { exe: Some(PathBuf::from("/opt/cvbuilder/cvbuilder")), ..env(other) };
        assert_eq!(plan(&inherited), None);
        // A folder that merely shares the prefix as text is not under APPDIR.
        let lookalike = Env { exe: Some(PathBuf::from("/tmp/.mount_CVBuiXyz/usr/bin/cvbuilder")), ..env(other) };
        assert_eq!(plan(&lookalike), None);
        assert_eq!(plan(&Env { appdir: None, ..env(other) }), None, "APPIMAGE without APPDIR");
        assert_eq!(plan(&Env { exe: None, ..env(other) }), None, "own path unknown");
        assert_eq!(plan(&Env { appdir: Some("/".into()), ..env(other) }), None, "APPDIR=/ proves nothing");
        assert_eq!(plan(&Env { appdir: Some("tmp/.mount_CVBuiXy".into()), ..env(other) }), None, "relative APPDIR");
        assert_eq!(plan(&Env { appimage_is_file: false, ..env(other) }), None, "APPIMAGE file is missing");
    }

    #[test]
    fn refuses_a_relative_home() {
        let a = Some("/a.AppImage");
        assert_eq!(plan(&Env { home: Some("home/kari".into()), ..env(a) }), None);
        assert_eq!(plan(&Env { home: Some("".into()), ..env(a) }), None);
        // An absolute XDG_DATA_HOME needs no HOME at all.
        let p = plan(&Env { home: Some("home/kari".into()), data_home: Some("/data".into()), ..env(a) }).unwrap();
        assert_eq!(p.icon_file, PathBuf::from("/data/icons/hicolor/256x256/apps/cvbuilder.png"));
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

    /// A fresh data folder for one test, and a plan that writes into it.
    fn test_plan(test: &str) -> (PathBuf, Plan) {
        let dir = std::env::temp_dir().join(format!("cvbuilder-appimage-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let plan = Plan { appimage: "/home/kari/CVBuilder.AppImage".into(), desktop_file: dir.join("applications/cvbuilder.desktop"), icon_file: dir.join("icons/hicolor/256x256/apps/cvbuilder.png") };
        (dir, plan)
    }

    /// Every file in `dir` and below, so a test can see that no temporary file is left.
    fn files_under(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if e.file_type().unwrap().is_dir() { out.extend(files_under(&p)) } else { out.push(p) }
        }
        out.sort();
        out
    }

    #[test]
    fn never_replaces_an_entry_that_is_not_utf8() {
        let (dir, plan) = test_plan("latin1");
        std::fs::create_dir_all(plan.desktop_file.parent().unwrap()).unwrap();
        let latin1 = b"[Desktop Entry]\nName=S\xf8knad\nX-CVBuilder-AppImage=true\n";
        std::fs::write(&plan.desktop_file, latin1).unwrap();
        assert_eq!(apply(&plan).unwrap(), Action::NotOurs);
        assert_eq!(std::fs::read(&plan.desktop_file).unwrap(), latin1);
        assert!(!plan.icon_file.exists(), "nothing is written for someone else's entry");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn never_replaces_an_entry_that_cannot_be_read() {
        let (dir, plan) = test_plan("unreadable");
        // A folder in the entry's place can't be read as a file on any system.
        std::fs::create_dir_all(plan.desktop_file.join("inside")).unwrap();
        assert!(apply(&plan).is_err());
        assert!(plan.desktop_file.join("inside").is_dir(), "left as it was");
        assert!(!plan.icon_file.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn never_replaces_an_entry_without_read_permission() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, plan) = test_plan("noperm");
        std::fs::create_dir_all(plan.desktop_file.parent().unwrap()).unwrap();
        let ours = desktop_entry("/old/CVBuilder.AppImage");
        std::fs::write(&plan.desktop_file, &ours).unwrap();
        std::fs::set_permissions(&plan.desktop_file, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&plan.desktop_file).is_ok() {
            // Running as root: permissions don't stop reading, so there is nothing to test.
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        assert!(apply(&plan).is_err());
        std::fs::set_permissions(&plan.desktop_file, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(std::fs::read_to_string(&plan.desktop_file).unwrap(), ours, "left as it was");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn never_writes_through_a_symlink() {
        let (dir, plan) = test_plan("symlink");
        std::fs::create_dir_all(plan.desktop_file.parent().unwrap()).unwrap();
        std::fs::create_dir_all(plan.icon_file.parent().unwrap()).unwrap();
        // A symlink in the entry's place, even to our own stale text, is someone else's setup.
        let target = dir.join("target.desktop");
        let stale = desktop_entry("/old/CVBuilder.AppImage");
        std::fs::write(&target, &stale).unwrap();
        std::os::unix::fs::symlink(&target, &plan.desktop_file).unwrap();
        assert_eq!(apply(&plan).unwrap(), Action::NotOurs);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), stale);
        assert!(std::fs::symlink_metadata(&plan.desktop_file).unwrap().file_type().is_symlink());
        // A symlink in the icon's place is left alone while the entry is written.
        std::fs::remove_file(&plan.desktop_file).unwrap();
        let icon_target = dir.join("target.png");
        std::fs::write(&icon_target, b"not the icon").unwrap();
        std::os::unix::fs::symlink(&icon_target, &plan.icon_file).unwrap();
        assert_eq!(apply(&plan).unwrap(), Action::Write);
        assert_eq!(std::fs::read(&icon_target).unwrap(), b"not the icon");
        assert!(std::fs::symlink_metadata(&plan.icon_file).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&plan.desktop_file).unwrap(), desktop_entry(&plan.appimage));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn temporary_files_are_new_and_unique() {
        let (dir, plan) = test_plan("tmp");
        std::fs::create_dir_all(plan.desktop_file.parent().unwrap()).unwrap();
        // The old fixed temporary name is no longer used, so a file planted there is never opened.
        let planted = plan.desktop_file.with_extension("tmp-cvbuilder");
        std::fs::write(&planted, b"planted").unwrap();
        write_atomic(&plan.desktop_file, b"one").unwrap();
        write_atomic(&plan.desktop_file, b"two").unwrap();
        assert_eq!(std::fs::read(&plan.desktop_file).unwrap(), b"two");
        assert_eq!(std::fs::read(&planted).unwrap(), b"planted");
        let mut names: Vec<String> = files_under(&dir).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, ["cvbuilder.desktop", "cvbuilder.tmp-cvbuilder"], "no temporary file left");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn written_files_are_readable_by_the_desktop() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, plan) = test_plan("mode");
        apply(&plan).unwrap();
        let mode = std::fs::metadata(&plan.desktop_file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode & 0o600, 0o600, "mode {mode:o}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_writes_icon_and_entry_then_keeps_them() {
        let (dir, plan) = test_plan("apply");
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
