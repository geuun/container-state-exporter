//! Docker API 클라이언트 (유닉스 소켓, GET만).

use serde_json::Value;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);

/// HTTP/1.0 GET을 보내고 (상태 코드, 본문)을 돌려준다. 1.0이라 Docker가 chunked 없이 본문을 주고 연결을 닫는다.
pub fn docker_get(socket: &str, path: &str) -> Result<(u16, Vec<u8>), String> {
    let mut s = UnixStream::connect(socket).map_err(|e| format!("connect {socket}: {e}"))?;
    s.set_read_timeout(Some(TIMEOUT)).ok();
    s.set_write_timeout(Some(TIMEOUT)).ok();
    write!(s, "GET {path} HTTP/1.0\r\nHost: docker\r\n\r\n").map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or("malformed response")?;
    let head = String::from_utf8_lossy(&raw[..split]);
    let code = head
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or("malformed status line")?;
    Ok((code, raw[split + 4..].to_vec()))
}

pub fn docker_json(socket: &str, path: &str) -> Result<Value, String> {
    let (code, body) = docker_get(socket, path)?;
    if code != 200 {
        return Err(format!("GET {path}: {code}"));
    }
    serde_json::from_slice(&body).map_err(|e| format!("GET {path}: {e}"))
}
