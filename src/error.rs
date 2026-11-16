#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("socket enumeration failed: {0}")]
    Enumerate(#[from] netstat2::error::Error),

    #[error("/proc is not readable; sockbook requires Linux procfs")]
    NoProc,

    #[error("unsupported knowledge-base schema {0}")]
    Schema(u32),

    #[error("invalid knowledge-base data: {0}")]
    Data(String),

    #[allow(dead_code)]
    #[error("web support is not compiled in; rebuild with --features web, or use --file")]
    WebDisabled,

    #[cfg(feature = "web")]
    #[error("HTTP update failed: {0}")]
    Http(String),

    #[error("cannot determine cache directory: set XDG_CACHE_HOME or HOME")]
    NoCacheDir,

    #[error("{0}")]
    Message(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
