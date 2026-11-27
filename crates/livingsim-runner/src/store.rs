//! Reading previously published files from a URL (GitHub Pages) or a local directory.

use std::path::PathBuf;

pub enum Source {
    Http(String),
    Dir(PathBuf),
    None,
}

impl Source {
    pub fn parse(s: Option<&str>) -> Source {
        match s {
            None | Some("") => Source::None,
            Some(s) if s.starts_with("http://") || s.starts_with("https://") => {
                Source::Http(s.trim_end_matches('/').to_string())
            }
            Some(s) => Source::Dir(PathBuf::from(s)),
        }
    }

    /// Fetches a file; `Ok(None)` if it does not exist.
    pub fn get(&self, path: &str) -> Result<Option<Vec<u8>>, String> {
        match self {
            Source::None => Ok(None),
            Source::Dir(d) => {
                let p = d.join(path);
                match std::fs::read(&p) {
                    Ok(b) => Ok(Some(b)),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                    Err(e) => Err(format!("{}: {e}", p.display())),
                }
            }
            Source::Http(base) => {
                let url = format!("{base}/{path}");
                fetch_url(&url)
            }
        }
    }
}

pub fn fetch_url(url: &str) -> Result<Option<Vec<u8>>, String> {
    let mut last = String::new();
    for attempt in 0..4 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(2 << attempt));
        }
        match ureq::get(url).header("Cache-Control", "no-cache").call() {
            Ok(mut resp) => {
                return resp
                    .body_mut()
                    .with_config()
                    .limit(1 << 30)
                    .read_to_vec()
                    .map(Some)
                    .map_err(|e| format!("{url}: {e}"));
            }
            Err(ureq::Error::StatusCode(404)) => return Ok(None),
            Err(e) => last = format!("{url}: {e}"),
        }
    }
    Err(last)
}

/// Reads a local path or URL (used for `--restore`).
pub fn read_any(s: &str) -> Result<Option<Vec<u8>>, String> {
    if s.starts_with("http://") || s.starts_with("https://") {
        fetch_url(s)
    } else {
        match std::fs::read(s) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{s}: {e}")),
        }
    }
}
