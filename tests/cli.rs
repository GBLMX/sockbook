use std::fs;
use std::net::{TcpListener, UdpSocket};
use std::process::Output;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

fn command(dir: &TempDir) -> Command {
    let mut command = Command::cargo_bin("sockbook").expect("sockbook binary");
    command
        .env("XDG_CACHE_HOME", dir.path().join("cache"))
        .env("XDG_CONFIG_HOME", dir.path().join("config"));
    command
}

fn parse_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "invalid JSON: {err}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn lists_own_tcp_listener_with_owner() {
    let dir = TempDir::new().unwrap();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();

    let output = command(&dir)
        .args(["--json", "list", "--tcp", "--port", &port.to_string()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let payload = parse_json(&output);
    let sockets = payload["sockets"].as_array().unwrap();
    let pid = std::process::id() as u64;
    let found = sockets.iter().any(|socket| {
        socket["proto"] == "tcp"
            && socket["state"] == "listen"
            && socket["local"]["port"].as_u64() == Some(port as u64)
            && socket["owners"]
                .as_array()
                .unwrap()
                .iter()
                .any(|owner| owner["pid"].as_u64() == Some(pid))
    });
    assert!(found, "listener not found with owner:\n{payload}");
}

#[test]
fn lists_own_udp_socket_with_owner() {
    let dir = TempDir::new().unwrap();
    let socket = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
    let port = socket.local_addr().unwrap().port();

    let output = command(&dir)
        .args(["--json", "list", "--udp", "--port", &port.to_string()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let payload = parse_json(&output);
    let sockets = payload["sockets"].as_array().unwrap();
    let pid = std::process::id() as u64;
    let found = sockets.iter().any(|socket| {
        socket["proto"] == "udp"
            && socket["state"] == "unconn"
            && socket["local"]["port"].as_u64() == Some(port as u64)
            && socket["owners"]
                .as_array()
                .unwrap()
                .iter()
                .any(|owner| owner["pid"].as_u64() == Some(pid))
    });
    assert!(found, "UDP socket not found with owner:\n{payload}");
}

#[test]
fn lookup_ranks_curated_before_iana() {
    let dir = TempDir::new().unwrap();
    let output = command(&dir)
        .args(["--json", "lookup", "8000"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let payload = parse_json(&output);
    let matches = payload["matches"].as_array().unwrap();
    assert!(matches.len() >= 2);
    assert_eq!(matches[0]["tier"], "curated");
    assert_eq!(matches[0]["name"], "dev-http");
    assert!(matches.iter().any(|entry| entry["tier"] == "iana"));
}

#[test]
fn lookup_missing_exits_one_with_empty_json() {
    let dir = TempDir::new().unwrap();
    let output = command(&dir)
        .args(["--json", "lookup", "65530"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let payload = parse_json(&output);
    assert!(payload["matches"].as_array().unwrap().is_empty());
}

#[test]
fn bad_cache_falls_back_to_builtin_with_warning() {
    let dir = TempDir::new().unwrap();
    let cache_dir = dir.path().join("cache").join("sockbook");
    fs::create_dir_all(&cache_dir).unwrap();
    fs::write(cache_dir.join("ports.json"), b"not json").unwrap();

    let output = command(&dir)
        .args(["--json", "lookup", "5432"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ignoring cache"), "stderr: {stderr}");

    let payload = parse_json(&output);
    assert_eq!(payload["matches"][0]["name"], "postgresql");
}

#[test]
fn update_from_file_round_trips_through_cache() {
    let dir = TempDir::new().unwrap();
    let payload_path = dir.path().join("payload.json");
    fs::write(
        &payload_path,
        r#"{
            "schema": 1,
            "source": "integration-test",
            "entries": [{
                "port": 54321,
                "protos": ["tcp"],
                "name": "test-svc",
                "description": "test service",
                "category": "other"
            }]
        }"#,
    )
    .unwrap();

    let output = command(&dir)
        .args(["db", "update", "--file"])
        .arg(&payload_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir
        .path()
        .join("cache")
        .join("sockbook")
        .join("ports.json")
        .is_file());

    let output = command(&dir)
        .args(["--json", "lookup", "54321"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let payload = parse_json(&output);
    assert_eq!(payload["matches"][0]["name"], "test-svc");
    assert_eq!(payload["matches"][0]["tier"], "curated");
}

#[test]
fn update_rejects_bad_schema_without_writing_cache() {
    let dir = TempDir::new().unwrap();
    let payload_path = dir.path().join("payload.json");
    fs::write(&payload_path, r#"{"schema":99,"entries":[]}"#).unwrap();

    let output = command(&dir)
        .args(["db", "update", "--file"])
        .arg(&payload_path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported"));
    assert!(!dir
        .path()
        .join("cache")
        .join("sockbook")
        .join("ports.json")
        .exists());
}

#[cfg(not(feature = "web"))]
#[test]
fn update_without_web_points_at_file() {
    let dir = TempDir::new().unwrap();
    let output = command(&dir).args(["db", "update"]).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("web support is not compiled in"),
        "{stderr}"
    );
    assert!(stderr.contains("--file"), "{stderr}");
}

#[test]
fn search_and_browse_return_structured_json() {
    let dir = TempDir::new().unwrap();

    let output = command(&dir)
        .args(["--json", "search", "redis"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let payload = parse_json(&output);
    assert_eq!(payload["query"], "redis");
    assert!(!payload["matches"].as_array().unwrap().is_empty());

    let output = command(&dir)
        .args(["--json", "browse", "--category", "database", "--limit", "5"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let payload = parse_json(&output);
    let entries = payload["entries"].as_array().unwrap();
    assert!(!entries.is_empty());
    assert!(entries.len() <= 5);
    assert!(payload["total"].as_u64().unwrap() >= entries.len() as u64);
    assert!(entries.iter().all(|entry| entry["category"] == "database"));
}

#[test]
fn user_overrides_take_precedence_and_survive_update() {
    let dir = TempDir::new().unwrap();
    let config_dir = dir.path().join("config").join("sockbook");
    fs::create_dir_all(&config_dir).unwrap();
    let overrides_path = config_dir.join("overrides.json");
    let overrides = r#"{
        "schema": 1,
        "entries": [{
            "port": 5432,
            "protos": ["tcp"],
            "name": "postgresql",
            "description": "local postgresql override",
            "notes": "local"
        }]
    }"#;
    fs::write(&overrides_path, overrides).unwrap();

    let output = command(&dir)
        .args(["--json", "lookup", "5432"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let payload = parse_json(&output);
    assert_eq!(payload["matches"][0]["tier"], "user");
    assert_eq!(
        payload["matches"][0]["description"],
        "local postgresql override"
    );

    let curated = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/curated.json");
    let output = command(&dir)
        .args(["db", "update", "--file"])
        .arg(&curated)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(&overrides_path).unwrap(), overrides);

    let output = command(&dir)
        .args(["--json", "lookup", "5432"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let payload = parse_json(&output);
    assert_eq!(payload["matches"][0]["tier"], "user");
}

#[test]
fn search_missing_exits_one_with_empty_json() {
    let dir = TempDir::new().unwrap();
    let output = command(&dir)
        .args(["--json", "search", "zzzz-no-such-service"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let payload = parse_json(&output);
    assert_eq!(payload["total"], 0);
    assert!(payload["matches"].as_array().unwrap().is_empty());
}

#[test]
fn update_json_reports_installed_path() {
    let dir = TempDir::new().unwrap();
    let payload_path = dir.path().join("payload.json");
    fs::write(
        &payload_path,
        r#"{
            "schema": 1,
            "entries": [{
                "port": 54321,
                "protos": ["tcp"],
                "name": "test-svc",
                "description": "test service"
            }]
        }"#,
    )
    .unwrap();

    let output = command(&dir)
        .args(["--json", "db", "update", "--file"])
        .arg(&payload_path)
        .output()
        .unwrap();
    assert!(output.status.success());
    let payload = parse_json(&output);
    assert_eq!(payload["updated"], true);
    assert_eq!(payload["entries"], 1);
    assert!(payload["path"]
        .as_str()
        .unwrap()
        .ends_with("sockbook/ports.json"));
}
