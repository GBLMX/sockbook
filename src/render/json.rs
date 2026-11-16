use std::net::IpAddr;
use std::path::Path;

use serde::Serialize;

use crate::error::Result;
use crate::model::{Owner, Proto, ServiceEntry, SocketView, State};

const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize)]
struct AddrPort {
    addr: IpAddr,
    port: u16,
}

#[derive(Serialize)]
struct JsonSocket<'a> {
    proto: Proto,
    state: State,
    local: AddrPort,
    remote: AddrPort,
    inode: u32,
    uid: u32,
    owners: &'a [Owner],
    hints: &'a [ServiceEntry],
}

#[derive(Serialize)]
struct SocketList<'a> {
    schema_version: u32,
    sockets: Vec<JsonSocket<'a>>,
}

pub fn sockets(views: &[SocketView]) -> Result<String> {
    let payload = SocketList {
        schema_version: SCHEMA_VERSION,
        sockets: views
            .iter()
            .map(|view| JsonSocket {
                proto: view.socket.proto,
                state: view.socket.state,
                local: AddrPort {
                    addr: view.socket.local_addr,
                    port: view.socket.local_port,
                },
                remote: AddrPort {
                    addr: view.socket.remote_addr,
                    port: view.socket.remote_port,
                },
                inode: view.socket.inode,
                uid: view.socket.uid,
                owners: &view.socket.owners,
                hints: &view.hints,
            })
            .collect(),
    };
    Ok(serde_json::to_string_pretty(&payload)?)
}

#[derive(Serialize)]
struct LookupResponse {
    schema_version: u32,
    port: u16,
    matches: Vec<ServiceEntry>,
}

pub fn lookup(port: u16, matches: &[&ServiceEntry]) -> Result<String> {
    let payload = LookupResponse {
        schema_version: SCHEMA_VERSION,
        port,
        matches: clone_entries(matches),
    };
    Ok(serde_json::to_string_pretty(&payload)?)
}

#[derive(Serialize)]
struct SearchResponse {
    schema_version: u32,
    query: String,
    total: usize,
    matches: Vec<ServiceEntry>,
}

pub fn search(query: &str, total: usize, matches: &[&ServiceEntry]) -> Result<String> {
    let payload = SearchResponse {
        schema_version: SCHEMA_VERSION,
        query: query.to_string(),
        total,
        matches: clone_entries(matches),
    };
    Ok(serde_json::to_string_pretty(&payload)?)
}

#[derive(Serialize)]
struct BrowseResponse {
    schema_version: u32,
    total: usize,
    offset: usize,
    limit: usize,
    entries: Vec<ServiceEntry>,
}

pub fn browse(
    total: usize,
    offset: usize,
    limit: usize,
    entries: &[&ServiceEntry],
) -> Result<String> {
    let payload = BrowseResponse {
        schema_version: SCHEMA_VERSION,
        total,
        offset,
        limit,
        entries: clone_entries(entries),
    };
    Ok(serde_json::to_string_pretty(&payload)?)
}

#[derive(Serialize)]
struct UpdateResponse<'a> {
    schema_version: u32,
    updated: bool,
    path: &'a str,
    entries: usize,
    source: Option<&'a str>,
}

pub fn update(path: &Path, entries: usize, source: Option<&str>) -> Result<String> {
    let path = path.to_string_lossy();
    let payload = UpdateResponse {
        schema_version: SCHEMA_VERSION,
        updated: true,
        path: &path,
        entries,
        source,
    };
    Ok(serde_json::to_string_pretty(&payload)?)
}

fn clone_entries(entries: &[&ServiceEntry]) -> Vec<ServiceEntry> {
    entries.iter().map(|entry| (*entry).clone()).collect()
}
