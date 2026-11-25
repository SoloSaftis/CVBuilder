//! Errors in plain words. A message says what happened in everyday words and what the person can
//! do next; the technical text (such as «Access is denied. (os error 5)») goes on a line of its
//! own after [`DETAILS`], which the app shows as a quiet «Details» line or in a tooltip.
//!
//! [`file_cause`] and [`io_cause`] turn the technical text of a failed file or connection into
//! one of the [`CAUSES`], a whole sentence the app translates.

/// Starts the line with the technical text, under the message.
pub const DETAILS: &str = "Detaljer: ";

/// No access to the file or folder.
pub const NO_ACCESS: &str = "CVBuilder har ikke tilgang til filen eller mappen. Velg en annen mappe, for eksempel Dokumenter.";
/// Another program has the file open.
pub const IN_USE: &str = "Filen er åpen i et annet program. Lukk den der og prøv igjen.";
/// The file or folder is gone.
pub const GONE: &str = "Filen eller mappen finnes ikke lenger. Den kan være flyttet eller slettet.";
/// The disk is full.
pub const DISK_FULL: &str = "Det er ikke nok plass på disken. Frigjør litt plass, eller lagre et annet sted.";
/// The place can only be read (a locked memory stick, a CD, a shared folder without write access).
pub const READ_ONLY: &str = "Det går ikke an å lagre her. Velg et annet sted.";
/// The name or the path is too long, or has characters that are not allowed.
pub const BAD_NAME: &str = "Filnavnet er for langt eller har tegn som ikke er tillatt. Velg et kortere navn.";
/// The file is not text CVBuilder can read, or its content is not what CVBuilder expects.
pub const DAMAGED: &str = "Innholdet er ikke slik CVBuilder venter. Filen kan være skadet eller laget for noe annet.";
/// No connection.
pub const OFFLINE: &str = "Fikk ikke kontakt over internett. Sjekk internettforbindelsen og prøv igjen.";
/// The answer took too long.
pub const TOO_SLOW: &str = "Det tok for lang tid å få svar. Sjekk internettforbindelsen og prøv igjen.";

/// What to try when the cause of a failed read is unknown.
pub const TRY_ANOTHER_FILE: &str = "Prøv igjen, eller velg en annen fil.";
/// What to try when the cause of a failed save is unknown.
pub const TRY_ANOTHER_PLACE: &str = "Prøv igjen, eller lagre et annet sted.";

/// What to try when CVBuilder's own file (in its data folder) could not be saved.
pub const TRY_AGAIN: &str = "Prøv igjen. Hjelper det ikke, start CVBuilder på nytt.";

/// Every cause, so the app can translate a message that ends with one.
pub const CAUSES: &[&str] = &[NO_ACCESS, IN_USE, GONE, DISK_FULL, READ_ONLY, BAD_NAME, DAMAGED, OFFLINE, TOO_SLOW, TRY_ANOTHER_FILE, TRY_ANOTHER_PLACE, TRY_AGAIN];

/// `text`, then the technical text on a [`DETAILS`] line (left out when empty).
pub fn with_details(text: &str, technical: &str) -> String {
    let technical = technical.trim();
    if technical.is_empty() { text.to_string() } else { format!("{text}\n{DETAILS}{technical}") }
}

/// The message and its technical text, when it has a [`DETAILS`] line.
pub fn split(text: &str) -> (&str, Option<&str>) {
    match text.split_once(&format!("\n{DETAILS}")) {
        Some((main, details)) => (main, Some(details)),
        None => (text, None),
    }
}

/// The OS error number in Rust's «… (os error 5)».
fn os_code(technical: &str) -> Option<i32> {
    let at = technical.rfind("(os error ")?;
    technical[at + 10..].split(')').next()?.trim().parse().ok()
}

fn kind_cause(kind: std::io::ErrorKind) -> Option<&'static str> {
    use std::io::ErrorKind as K;
    Some(match kind {
        K::PermissionDenied => NO_ACCESS,
        K::NotFound => GONE,
        K::ResourceBusy | K::ExecutableFileBusy => IN_USE,
        K::StorageFull | K::QuotaExceeded | K::FileTooLarge => DISK_FULL,
        K::ReadOnlyFilesystem => READ_ONLY,
        K::InvalidFilename => BAD_NAME,
        K::InvalidData => DAMAGED,
        K::TimedOut => TOO_SLOW,
        K::ConnectionRefused | K::ConnectionReset | K::ConnectionAborted | K::NotConnected | K::NetworkUnreachable | K::HostUnreachable | K::NetworkDown | K::AddrNotAvailable => OFFLINE,
        _ => return None,
    })
}

/// Why a file could not be read or written, from its error.
pub fn io_cause(error: &std::io::Error) -> Option<&'static str> {
    file_cause(&error.to_string()).or_else(|| kind_cause(error.kind()))
}

/// Why a file could not be read or written, from the technical text of its error. `None` when
/// the text says nothing CVBuilder knows; the caller then says what to try in its own words.
pub fn file_cause(technical: &str) -> Option<&'static str> {
    if let Some(code) = os_code(technical) {
        // The file is open in another program (a sharing or lock violation).
        if cfg!(windows) && matches!(code, 32 | 33) {
            return Some(IN_USE);
        }
        if cfg!(windows) && code == 19 {
            return Some(READ_ONLY);
        }
        if let Some(cause) = kind_cause(std::io::Error::from_raw_os_error(code).kind()) {
            return Some(cause);
        }
    }
    let lower = technical.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    if has(&["permission denied", "access is denied", "operation not permitted"]) {
        Some(NO_ACCESS)
    } else if has(&["used by another process", "resource busy", "locked a portion"]) {
        Some(IN_USE)
    } else if has(&["no such file", "cannot find the file", "cannot find the path", "entity not found"]) {
        Some(GONE)
    } else if has(&["no space left", "not enough space", "disk full", "storage full"]) {
        Some(DISK_FULL)
    } else if has(&["read-only file system", "write protected", "write-protected"]) {
        Some(READ_ONLY)
    } else if has(&["file name too long", "filename too long", "invalid filename", "syntax is incorrect"]) {
        Some(BAD_NAME)
    } else if has(&["valid utf-8", "invalid utf-8", "eof while parsing", "expected value", "invalid type", "missing field", "unknown variant", "trailing characters", "key must be a string", "control character"]) {
        Some(DAMAGED)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_say_why_in_plain_words() {
        let real = |kind: std::io::ErrorKind| io_cause(&std::io::Error::from(kind));
        assert_eq!(real(std::io::ErrorKind::PermissionDenied), Some(NO_ACCESS));
        assert_eq!(real(std::io::ErrorKind::NotFound), Some(GONE));
        assert_eq!(real(std::io::ErrorKind::StorageFull), Some(DISK_FULL));
        assert_eq!(real(std::io::ErrorKind::Other), None);
        // The OS's own words are in the person's language; the number is not.
        let os = |code: i32| file_cause(&std::io::Error::from_raw_os_error(code).to_string());
        if cfg!(windows) {
            assert_eq!(os(5), Some(NO_ACCESS));
            assert_eq!(os(2), Some(GONE));
            assert_eq!(os(3), Some(GONE));
            assert_eq!(os(32), Some(IN_USE));
            assert_eq!(os(112), Some(DISK_FULL));
            assert_eq!(os(206), Some(BAD_NAME));
            assert_eq!(file_cause("Ingen tilgang. (os error 5)"), Some(NO_ACCESS));
        } else {
            assert_eq!(os(13), Some(NO_ACCESS));
            assert_eq!(os(2), Some(GONE));
            assert_eq!(os(28), Some(DISK_FULL));
            assert_eq!(os(30), Some(READ_ONLY));
            assert_eq!(os(36), Some(BAD_NAME));
        }
        assert_eq!(file_cause("Permission denied"), Some(NO_ACCESS));
        assert_eq!(file_cause("expected value at line 1 column 1"), Some(DAMAGED));
        assert_eq!(file_cause("stream did not contain valid UTF-8"), Some(DAMAGED));
        assert_eq!(file_cause("something odd"), None);
    }

    #[test]
    fn details_go_on_their_own_line() {
        let text = with_details("Filen ble ikke lagret.", " Access is denied. (os error 5) ");
        assert_eq!(text, "Filen ble ikke lagret.\nDetaljer: Access is denied. (os error 5)");
        assert_eq!(split(&text), ("Filen ble ikke lagret.", Some("Access is denied. (os error 5)")));
        assert_eq!(with_details("Lagret.", "  "), "Lagret.");
        assert_eq!(split("Lagret."), ("Lagret.", None));
    }
}
