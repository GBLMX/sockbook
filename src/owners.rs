use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;

use crate::model::Owner;

const CMDLINE_LIMIT: u64 = 8192;

#[derive(Default)]
pub struct OwnerResolver {
    cache: HashMap<u32, Option<Owner>>,
}

impl OwnerResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn resolve(&mut self, pids: &[u32]) -> Vec<Owner> {
        let mut owners: Vec<Owner> = pids
            .iter()
            .filter_map(|pid| self.lookup(*pid).cloned())
            .collect();
        owners.sort_by_key(|owner| owner.pid);
        owners.dedup_by_key(|owner| owner.pid);
        owners
    }

    fn lookup(&mut self, pid: u32) -> Option<&Owner> {
        self.cache
            .entry(pid)
            .or_insert_with(|| read_owner(pid))
            .as_ref()
    }
}

pub fn read_owner(pid: u32) -> Option<Owner> {
    let mut cmdline_bytes = Vec::new();
    File::open(format!("/proc/{pid}/cmdline"))
        .ok()
        .and_then(|file| {
            file.take(CMDLINE_LIMIT)
                .read_to_end(&mut cmdline_bytes)
                .ok()
        });

    let mut cmdline = String::from_utf8_lossy(&cmdline_bytes)
        .replace('\0', " ")
        .trim()
        .to_string();

    if cmdline.is_empty() {
        cmdline = fs::read_to_string(format!("/proc/{pid}/comm"))
            .map(|comm| comm.trim().to_string())
            .unwrap_or_default();
    }

    let exe = fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .map(|path| path.to_string_lossy().into_owned());

    if cmdline.is_empty() && exe.is_none() {
        return None;
    }

    Some(Owner { pid, cmdline, exe })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_current_process_owner() {
        let owner = read_owner(std::process::id()).expect("current process should be readable");
        assert_eq!(owner.pid, std::process::id());
        assert!(!owner.cmdline.is_empty());
    }
}
