use std::env;
use std::path::PathBuf;

fn xdg_dir(variable: &str, fallback: &str) -> Option<PathBuf> {
    if let Some(value) = env::var_os(variable) {
        if !value.is_empty() {
            return Some(PathBuf::from(value));
        }
    }
    env::var_os("HOME").map(|home| PathBuf::from(home).join(fallback))
}

pub fn cache_file() -> Option<PathBuf> {
    xdg_dir("XDG_CACHE_HOME", ".cache").map(|dir| dir.join("sockbook").join("ports.json"))
}

pub fn overrides_file() -> Option<PathBuf> {
    xdg_dir("XDG_CONFIG_HOME", ".config").map(|dir| dir.join("sockbook").join("overrides.json"))
}
