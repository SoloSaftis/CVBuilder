//! Fetching a job ad from a link: redirects, pages that are not ads, errors and time limits,
//! against a small web server on this machine (no internet needed).
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use cv_ai::providers::fetch_page_within;

/// A web server on 127.0.0.1 that answers by path; returns its address.
fn server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = format!("http://{}", listener.local_addr().expect("addr"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut stream = stream;
                let mut request = Vec::new();
                let mut buf = [0u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => request.extend_from_slice(&buf[..n]),
                    }
                }
                let line = String::from_utf8_lossy(&request).lines().next().unwrap_or_default().to_string();
                let path = line.split(' ').nth(1).unwrap_or("/").to_string();
                let ad = r#"<html><head><script type="application/ld+json">{"@type":"JobPosting","title":"Kokk","hiringOrganization":"Eksempel Kro AS","description":"Lag mat fra bunnen av."}</script></head><body>Kokk</body></html>"#;
                let (status, kind, body): (&str, &str, Vec<u8>) = match path.as_str() {
                    "/ad" => ("200 OK", "text/html; charset=utf-8", ad.as_bytes().to_vec()),
                    "/redirect" => ("302 Found", "text/html", Vec::new()),
                    "/loop" => ("302 Found", "text/html", Vec::new()),
                    "/pdf" => ("200 OK", "application/pdf", b"%PDF-1.4 ...".to_vec()),
                    "/pdf-as-html" => ("200 OK", "text/html", b"%PDF-1.4 ...".to_vec()),
                    "/image" => ("200 OK", "image/png", vec![0x89, b'P', b'N', b'G']),
                    "/forbidden" => ("403 Forbidden", "text/html", b"nei".to_vec()),
                    "/gone" => ("404 Not Found", "text/html", b"borte".to_vec()),
                    "/busy" => ("429 Too Many Requests", "text/html", Vec::new()),
                    "/broken" => ("500 Internal Server Error", "text/html", Vec::new()),
                    "/latin" => ("200 OK", "text/html; charset=iso-8859-1", b"<p>S\xf8knadsfrist for kokk p\xe5 kroa</p>".to_vec()),
                    "/plain" => ("200 OK", "text/plain; charset=utf-8", "Stilling: Kokk <hovedkokk>\nArbeidsgiver: Eksempel Kro AS\n\nVi lager mat fra bunnen av og trenger en kokk med fagbrev og erfaring fra travle kjøkken.".as_bytes().to_vec()),
                    "/huge" => ("200 OK", "text/html", vec![b'a'; 6_000_000]),
                    "/slow" => {
                        std::thread::sleep(Duration::from_secs(4));
                        ("200 OK", "text/html", ad.as_bytes().to_vec())
                    }
                    _ => ("404 Not Found", "text/html", Vec::new()),
                };
                let location = match path.as_str() {
                    "/redirect" => "Location: /ad\r\n",
                    "/loop" => "Location: /loop\r\n",
                    _ => "",
                };
                let head = format!("HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n{location}Connection: close\r\n\r\n", body.len());
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
            });
        }
    });
    addr
}

fn fetch(url: &str) -> Result<String, String> {
    fetch_page_within(url, Duration::from_secs(2))
}

#[test]
fn a_page_is_fetched_and_redirects_are_followed() {
    let base = server();
    let page = fetch(&format!("{base}/ad")).expect("ad");
    let ad = cv_core::import::ad_from_page(&page).expect("job data");
    assert_eq!((ad.role.as_str(), ad.company.as_str()), ("Kokk", "Eksempel Kro AS"));
    assert!(fetch(&format!("{base}/redirect")).expect("redirect").contains("JobPosting"));
    assert!(fetch(&format!("  {base}/ad  ")).is_ok(), "spaces around a pasted link are fine");
}

#[test]
fn pages_in_other_encodings_and_plain_text() {
    let base = server();
    let latin = fetch(&format!("{base}/latin")).expect("latin");
    assert!(cv_core::import::strip_html(&latin).contains("Søknadsfrist for kokk på kroa"), "{latin}");
    let plain = cv_core::import::ad_from_page(&fetch(&format!("{base}/plain")).expect("plain")).expect("ad");
    assert_eq!((plain.role.as_str(), plain.company.as_str()), ("Kokk <hovedkokk>", "Eksempel Kro AS"), "«<» in plain text is text, not a tag");
}

#[test]
fn every_failure_is_a_clear_message() {
    let base = server();
    let cases = [
        ("/loop", "for mange ganger"),
        ("/pdf", "PDF"),
        ("/pdf-as-html", "PDF"),
        ("/image", "ikke til en nettside"),
        ("/forbidden", "innlogging"),
        ("/gone", "404"),
        ("/busy", "for mange forespørsler"),
        ("/broken", "500"),
        ("/huge", "for stor"),
        ("/slow", "sekunder"),
    ];
    for (path, says) in cases {
        let start = Instant::now();
        let err = fetch(&format!("{base}{path}")).expect_err(path);
        assert!(err.contains(says), "{path}: {err}");
        assert!(start.elapsed() < Duration::from_secs(10), "{path} took {:?}", start.elapsed());
    }
}

#[test]
fn links_without_or_with_the_wrong_scheme() {
    assert!(fetch("").expect_err("empty").contains("Lim inn"));
    assert!(fetch("ftp://example.com/ad").expect_err("ftp").contains("https://"));
    assert!(fetch("bare noen ord").expect_err("words").contains("https://"));
    assert!(fetch("annonse").expect_err("one word").contains("https://"));
    // «www.…» without a scheme gets https:// (here nothing answers, so it fails cleanly).
    let start = Instant::now();
    assert!(fetch("127.0.0.1:1/ad").is_err());
    assert!(start.elapsed() < Duration::from_secs(10));
}
