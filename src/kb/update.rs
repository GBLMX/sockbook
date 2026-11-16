use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::kb::{parse_envelope, validate_envelope, Envelope};
use crate::paths;

pub const DEFAULT_UPDATE_URL: &str =
    "https://raw.githubusercontent.com/GBLMX/sockbook/main/data/curated.json";

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;

pub struct Installed {
    pub path: PathBuf,
    pub entries: usize,
    pub source: Option<String>,
}

pub fn install_file(path: &Path) -> Result<Installed> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_BYTES as u64 {
        return Err(Error::Data(format!(
            "{} is {} bytes; limit is {MAX_BYTES}",
            path.display(),
            metadata.len()
        )));
    }
    let bytes = fs::read(path)?;
    install_bytes(&bytes)
}

pub fn install_bytes(bytes: &[u8]) -> Result<Installed> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::Data(format!(
            "payload is {} bytes; limit is {MAX_BYTES}",
            bytes.len()
        )));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|err| Error::Data(format!("payload is not UTF-8: {err}")))?;
    let envelope: Envelope = parse_envelope(text, "update payload")?;

    if envelope.entries.is_empty() {
        return Err(Error::Data("update payload has no entries".to_string()));
    }
    if envelope.entries.len() > MAX_ENTRIES {
        return Err(Error::Data(format!(
            "update payload has {} entries; limit is {MAX_ENTRIES}",
            envelope.entries.len()
        )));
    }
    validate_envelope(&envelope)?;

    let path = paths::cache_file().ok_or(Error::NoCacheDir)?;
    write_atomic(&path, bytes)?;

    Ok(Installed {
        path,
        entries: envelope.entries.len(),
        source: envelope.source,
    })
}

#[cfg(feature = "web")]
pub fn fetch(url: &str) -> Result<Vec<u8>> {
    if !url.starts_with("https://") {
        return Err(Error::Http(
            "update URL must use https://; use --file for local payloads".to_string(),
        ));
    }
    let mut response = ureq::get(url)
        .call()
        .map_err(|err| Error::Http(err.to_string()))?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_BYTES as u64)
        .read_to_vec()
        .map_err(|err| Error::Http(err.to_string()))?;
    Ok(bytes)
}

#[cfg(not(feature = "web"))]
pub fn fetch(_url: &str) -> Result<Vec<u8>> {
    Err(Error::WebDisabled)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::Data(format!("invalid cache path {}", path.display())))?;
    fs::create_dir_all(parent)?;

    let temporary = parent.join(format!(".ports.json.tmp.{}", std::process::id()));
    {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }

    if let Err(err) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(err.into());
    }
    Ok(())
}
