//! Records which source the program was built from, for «Hjelp → Om CVBuilder»: the git commit
//! (short hash, «+endringer» when there were uncommitted changes), or GitHub Actions' commit.
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn main() {
    let build = match git(&["rev-parse", "--short=12", "HEAD"]) {
        Some(hash) if !hash.is_empty() => {
            let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
            if dirty { format!("{hash}+endringer") } else { hash }
        }
        _ => std::env::var("GITHUB_SHA").map(|s| s.chars().take(12).collect()).unwrap_or_else(|_| "ukjent".into()),
    };
    println!("cargo:rustc-env=CVBUILDER_BUILD={build}");
    // Build again when the commit changes.
    for path in ["../../.git/HEAD", "../../.git/index"] {
        println!("cargo:rerun-if-changed={path}");
    }
    if let Some(head) = std::fs::read_to_string("../../.git/HEAD").ok().and_then(|h| h.strip_prefix("ref: ").map(|r| r.trim().to_string())) {
        println!("cargo:rerun-if-changed=../../.git/{head}");
    }
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    bundled_languages();
}

/// The interface languages that ship with CVBuilder: every `<code>.cvlang.json` in
/// assets/languages, embedded as `BUNDLED_LANGUAGES` (code, file text).
fn bundled_languages() {
    let dir = std::path::Path::new("../../assets/languages");
    println!("cargo:rerun-if-changed=../../assets/languages");
    let mut files: Vec<std::path::PathBuf> =
        std::fs::read_dir(dir).map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.to_string_lossy().ends_with(".cvlang.json")).collect()).unwrap_or_default();
    files.sort();
    let mut src = String::from("pub static BUNDLED_LANGUAGES: &[(&str, &str)] = &[\n");
    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        let Ok(full) = path.canonicalize() else {
            continue;
        };
        let code = path.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".cvlang.json")).unwrap_or_default();
        src.push_str(&format!("    ({code:?}, include_str!({:?})),\n", full.display().to_string()));
    }
    src.push_str("];\n");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default()).join("bundled_languages.rs");
    if let Err(e) = std::fs::write(&out, src) {
        println!("cargo:warning=writing {}: {e}", out.display());
    }
}
