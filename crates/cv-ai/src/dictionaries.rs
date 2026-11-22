//! Downloads a spelling dictionary (to-do 046), only when the person clicks «Last ned ordboken».
//! The files come from one pinned commit on GitHub and must match their SHA-256 in
//! `cv_core::spelling::SOURCES`; nothing about the CV is sent. After that the check is offline.
use std::path::Path;
use std::time::Duration;

use cv_core::spelling::Source;
use sha2::{Digest, Sha256};

/// GitHub could not be reached (no connection, or it took too long).
pub const OFFLINE: &str = crate::updates::OFFLINE;
/// The file came, but not the one CVBuilder expects: it is not used.
pub const CHANGED: &str = "Ordboken som kom, var ikke den CVBuilder venter, og blir ikke brukt. Prøv igjen senere.";
/// GitHub answered with an error.
pub const REFUSED: &str = "GitHub ga ikke ut ordboken akkurat nå. Prøv igjen om noen minutter.";
/// The dictionary folder could not be written.
pub const NOT_SAVED: &str = "Ordboken kunne ikke lagres på maskinen.";

/// The SHA-256 of some bytes, as lower-case hex.
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn get(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, String> {
    let mut response = agent.get(url).call().map_err(|e| cv_core::plain::with_details(OFFLINE, &e.to_string()))?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(cv_core::plain::with_details(REFUSED, &format!("GitHub answered {status} for {url}")));
    }
    response.body_mut().with_config().limit(64 << 20).read_to_vec().map_err(|e| cv_core::plain::with_details(OFFLINE, &e.to_string()))
}

/// Downloads `source` into `dir` as `<id>.aff`, `<id>.dic` and `<id>.license.txt` (when it has
/// one), checked first. The message is for the person (a Norwegian key, see the constants).
pub fn download(source: &Source, dir: &Path) -> Result<(), String> {
    let agent =
        ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(120))).http_status_as_error(false).user_agent(concat!("CVBuilder/", env!("CARGO_PKG_VERSION"))).build().new_agent();
    let aff = get(&agent, &source.url("index.aff"))?;
    let dic = get(&agent, &source.url("index.dic"))?;
    for (bytes, want) in [(&aff, source.aff_sha256), (&dic, source.dic_sha256)] {
        let got = sha256(bytes);
        if got != want {
            return Err(cv_core::plain::with_details(CHANGED, &format!("SHA-256 {got}, expected {want}")));
        }
    }
    // The licence text is kept next to the files; a dictionary without one is still usable.
    let licence = get(&agent, &source.url("license")).ok();
    save(source.id, &aff, &dic, licence.as_deref(), dir)
}

/// Writes the files, the `.dic` last, so a half-written dictionary is never taken for a whole one.
pub fn save(id: &str, aff: &[u8], dic: &[u8], licence: Option<&[u8]>, dir: &Path) -> Result<(), String> {
    let failed = |e: std::io::Error| cv_core::plain::with_details(NOT_SAVED, &e.to_string());
    std::fs::create_dir_all(dir).map_err(failed)?;
    if let Some(text) = licence {
        std::fs::write(dir.join(format!("{id}.license.txt")), text).map_err(failed)?;
    }
    std::fs::write(dir.join(format!("{id}.aff")), aff).map_err(failed)?;
    let part = dir.join(format!("{id}.dic.part"));
    std::fs::write(&part, dic).map_err(failed)?;
    std::fs::rename(&part, dir.join(format!("{id}.dic"))).map_err(failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hash_is_sha_256() {
        assert_eq!(sha256(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn the_bundled_dictionaries_match_their_pinned_hashes() {
        // The bundled files are the same bytes as the download would be (gzipped).
        for s in cv_core::spelling::SOURCES.iter().filter(|s| s.bundled) {
            let (aff, dic) = cv_core::spelling::bundled(s.id).expect("bundled");
            let unzip = |b: &[u8]| cv_core::spelling::unpack(b).expect("gzip");
            assert_eq!(sha256(&unzip(aff)), s.aff_sha256, "{}", s.id);
            assert_eq!(sha256(&unzip(dic)), s.dic_sha256, "{}", s.id);
            assert_eq!((unzip(aff).len() + unzip(dic).len()) as u64, s.bytes, "{}", s.id);
        }
    }

    #[test]
    fn a_saved_dictionary_is_whole_or_not_there() {
        let dir = std::env::temp_dir().join(format!("cvbuilder-dict-{}", std::process::id()));
        save("xx", b"SET UTF-8\n", b"1\nhei\n", Some(b"MIT"), &dir).expect("save");
        assert!(dir.join("xx.dic").is_file() && dir.join("xx.aff").is_file() && dir.join("xx.license.txt").is_file());
        assert!(!dir.join("xx.dic.part").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
