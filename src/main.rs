//! container-state-exporter: Docker 컨테이너 상태(실행 여부, healthcheck, 재시작 횟수, OOM)를
//! Prometheus 형식으로 노출한다. cAdvisor는 healthcheck 상태를 주지 않아서 따로 둔다.
//! Docker API는 GET만 호출한다.
//!
//! 컨테이너 라벨
//!   monitoring.exclude=true   메트릭에서 뺀다
//!   monitoring.channel=<이름>  channel 라벨로 붙인다 (알림 채널 라우팅)
//!
//! 사용: `container-state-exporter` (서버) / `container-state-exporter healthcheck` (컨테이너 healthcheck용)

use serde_json::Value;
use std::env;
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::net::UnixStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const EXCLUDE_LABEL: &str = "monitoring.exclude";
const CHANNEL_LABEL: &str = "monitoring.channel";
const TIMEOUT: Duration = Duration::from_secs(10);

fn log(level: &str, msg: &str, kv: &[(&str, String)]) {
    let mut s = format!("level={level} msg={msg:?}");
    for (k, v) in kv {
        let _ = write!(s, " {k}={v:?}");
    }
    println!("{s}");
}

fn env_or(key: &str, def: &str) -> String {
    env::var(key).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| def.to_string())
}

// ------------------------------------------------------------------ Docker API (유닉스 소켓)

/// HTTP/1.0 GET을 보내고 (상태 코드, 본문)을 돌려준다. 1.0이라 Docker가 chunked 없이 본문을 주고 연결을 닫는다.
fn docker_get(socket: &str, path: &str) -> Result<(u16, Vec<u8>), String> {
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

fn docker_json(socket: &str, path: &str) -> Result<Value, String> {
    let (code, body) = docker_get(socket, path)?;
    if code != 200 {
        return Err(format!("GET {path}: {code}"));
    }
    serde_json::from_slice(&body).map_err(|e| format!("GET {path}: {e}"))
}

// ------------------------------------------------------------------ 메트릭

struct Row {
    labels: String,
    running: bool,
    /// "healthy" | "unhealthy" | "starting" | "none"
    health: String,
    restart_count: i64,
    oom_killed: bool,
    exit_code: i64,
    started_at: Option<i64>,
}

/// Prometheus 라벨 값 이스케이프: \ " 줄바꿈
fn escape(s: &str) -> String {
    s.replace('\\', r"\\").replace('"', "\\\"").replace('\n', r"\n")
}

/// compose가 재생성 중에 잠깐 붙이는 "<id 12자리>_이름" 컨테이너인지
fn is_transient(name: &str, id: &str) -> bool {
    name.len() > 13 && name.as_bytes()[12] == b'_' && id.starts_with(&name[..12])
}

/// Docker의 RFC3339 시각(나노초 포함 가능)을 Unix 초로. 0001-01-01(시작한 적 없음)이면 None.
fn parse_rfc3339(s: &str) -> Option<i64> {
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (num(0..4)?, num(5..7)?, num(8..10)?, num(11..13)?, num(14..16)?, num(17..19)?);
    if y < 2000 {
        return None;
    }
    // 시간대: Z 또는 ±hh:mm (소수 초는 건너뛴다)
    let rest = &s[19..];
    let tz = rest.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let offset = match tz.as_bytes().first() {
        Some(b'Z') | None => 0,
        Some(&sign) => {
            let oh: i64 = tz.get(1..3)?.parse().ok()?;
            let om: i64 = tz.get(4..6)?.parse().ok()?;
            let o = oh * 3600 + om * 60;
            if sign == b'-' { -o } else { o }
        }
    };
    // Howard Hinnant의 days_from_civil
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + se - offset)
}

fn collect(socket: &str) -> Result<Vec<Row>, String> {
    let list = docker_json(socket, "/containers/json?all=true")?;
    let mut rows = Vec::new();
    for c in list.as_array().into_iter().flatten() {
        let id = c["Id"].as_str().unwrap_or_default();
        let labels = &c["Labels"];
        let label = |k: &str| labels[k].as_str().unwrap_or_default();
        if label(EXCLUDE_LABEL) == "true" {
            continue;
        }
        let name = c["Names"][0].as_str().unwrap_or(id.get(..12).unwrap_or(id)).trim_start_matches('/');
        if is_transient(name, id) {
            continue;
        }
        // 목록을 받은 뒤 지워진 컨테이너는 건너뛴다.
        let Ok(info) = docker_json(socket, &format!("/containers/{id}/json")) else {
            continue;
        };
        let state = &info["State"];
        let mut l = format!(
            r#"container="{}",compose_project="{}",compose_service="{}",restart_policy="{}""#,
            escape(name),
            escape(label("com.docker.compose.project")),
            escape(label("com.docker.compose.service")),
            escape(info["HostConfig"]["RestartPolicy"]["Name"].as_str().unwrap_or_default()),
        );
        // 빈 라벨 값은 Prometheus에서 라벨이 없는 것과 같으므로 있을 때만 붙인다.
        if !label(CHANNEL_LABEL).is_empty() {
            let _ = write!(l, r#",channel="{}""#, escape(label(CHANNEL_LABEL)));
        }
        rows.push(Row {
            labels: l,
            running: state["Running"].as_bool().unwrap_or(false),
            health: state["Health"]["Status"].as_str().unwrap_or("none").to_string(),
            restart_count: info["RestartCount"].as_i64().unwrap_or(0),
            oom_killed: state["OOMKilled"].as_bool().unwrap_or(false),
            exit_code: state["ExitCode"].as_i64().unwrap_or(0),
            started_at: state["StartedAt"].as_str().and_then(parse_rfc3339),
        });
    }
    rows.sort_by(|a, b| a.labels.cmp(&b.labels));
    Ok(rows)
}

fn render(rows: &[Row]) -> String {
    let mut b = String::new();
    let mut metric = |name: &str, typ: &str, help: &str, value: &dyn Fn(&Row) -> Option<i64>| {
        let _ = writeln!(b, "# HELP {name} {help}\n# TYPE {name} {typ}");
        for r in rows {
            if let Some(v) = value(r) {
                let _ = writeln!(b, "{name}{{{}}} {v}", r.labels);
            }
        }
    };
    let has_hc = |r: &Row| r.health != "none";
    metric("docker_container_running", "gauge", "1 if the container is running.", &|r| Some(r.running as i64));
    metric("docker_container_has_healthcheck", "gauge", "1 if the container defines a healthcheck.", &|r| {
        Some(has_hc(r) as i64)
    });
    metric(
        "docker_container_healthy",
        "gauge",
        "1 healthy, 0 otherwise. Only for containers with a healthcheck.",
        &|r| has_hc(r).then(|| (r.health == "healthy") as i64),
    );
    metric("docker_container_unhealthy", "gauge", "1 if the healthcheck reports unhealthy.", &|r| {
        Some((r.health == "unhealthy") as i64)
    });
    metric("docker_container_restart_count", "counter", "Restarts done by the Docker restart policy.", &|r| {
        Some(r.restart_count)
    });
    metric("docker_container_oom_killed", "gauge", "1 if the last exit was an OOM kill.", &|r| {
        Some(r.oom_killed as i64)
    });
    metric("docker_container_exit_code", "gauge", "Exit code of the last run.", &|r| Some(r.exit_code));
    metric("docker_container_started_at_seconds", "gauge", "Unix time the container last started.", &|r| {
        r.started_at
    });
    b
}

// ------------------------------------------------------------------ HTTP 서버

fn respond(s: &mut TcpStream, code: u16, reason: &str, ctype: &str, body: &str) {
    let _ = write!(
        s,
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

fn handle(mut s: TcpStream, socket: &str) {
    s.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut line = String::new();
    if BufReader::new(&s).read_line(&mut line).is_err() {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("");
    match path {
        "/metrics" => match collect(socket) {
            Ok(rows) => respond(&mut s, 200, "OK", "text/plain; version=0.0.4; charset=utf-8", &render(&rows)),
            Err(e) => {
                log("ERROR", "collect failed", &[("err", e.clone())]);
                respond(&mut s, 502, "Bad Gateway", "text/plain", &format!("{e}\n"));
            }
        },
        "/healthz" => match docker_get(socket, "/_ping") {
            Ok((200, _)) => respond(&mut s, 200, "OK", "text/plain", "ok\n"),
            Ok((c, _)) => respond(&mut s, 503, "Service Unavailable", "text/plain", &format!("docker ping {c}\n")),
            Err(e) => respond(&mut s, 503, "Service Unavailable", "text/plain", &format!("{e}\n")),
        },
        _ => respond(&mut s, 404, "Not Found", "text/plain", "not found\n"),
    }
}

/// 컨테이너 안에 curl이 없어서 healthcheck는 같은 바이너리로 한다.
fn healthcheck(port: &str) -> bool {
    let Ok(mut s) = TcpStream::connect_timeout(&format!("127.0.0.1:{port}").parse().unwrap(), Duration::from_secs(3))
    else {
        return false;
    };
    s.set_read_timeout(Some(Duration::from_secs(10))).ok();
    if write!(s, "GET /healthz HTTP/1.0\r\n\r\n").is_err() {
        return false;
    }
    let mut first = String::new();
    BufReader::new(s).read_line(&mut first).is_ok() && first.split_whitespace().nth(1) == Some("200")
}

fn main() {
    let addr = env_or("LISTEN_ADDR", "0.0.0.0:9417");
    let socket = env_or("DOCKER_SOCKET", "/var/run/docker.sock");
    let port = addr.rsplit(':').next().unwrap_or("9417").to_string();

    if env::args().nth(1).as_deref() == Some("healthcheck") {
        std::process::exit(if healthcheck(&port) { 0 } else { 1 });
    }

    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        log("ERROR", "bind failed", &[("addr", addr.clone()), ("err", e.to_string())]);
        std::process::exit(1);
    });
    let started = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    log("INFO", "listening", &[("addr", addr), ("socket", socket.clone()), ("started", started.to_string())]);
    // 요청은 15초마다 한 번(Alloy) + healthcheck 정도라 순서대로 처리한다.
    for conn in listener.incoming().flatten() {
        handle(conn, &socket);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339() {
        assert_eq!(parse_rfc3339("2026-11-20T02:51:41.329482Z"), Some(1_795_143_101));
        assert_eq!(parse_rfc3339("2026-11-20T11:51:41+09:00"), Some(1_795_143_101));
        assert_eq!(parse_rfc3339("0001-01-01T00:00:00Z"), None);
        assert_eq!(parse_rfc3339(""), None);
    }

    #[test]
    fn transient() {
        assert!(is_transient("86986ad2f8f3_monitoring-loki-1", "86986ad2f8f38c497fc9b8be"));
        assert!(!is_transient("monitoring-loki-1", "86986ad2f8f38c497fc9b8be"));
        assert!(!is_transient("aaaaaaaaaaaa_x", "bbbbbbbbbbbbbbbb"));
    }

    #[test]
    fn render_rows() {
        let rows = vec![Row {
            labels: r#"container="a""#.into(),
            running: true,
            health: "none".into(),
            restart_count: 2,
            oom_killed: false,
            exit_code: 0,
            started_at: None,
        }];
        let out = render(&rows);
        assert!(out.contains("docker_container_running{container=\"a\"} 1\n"));
        assert!(out.contains("docker_container_restart_count{container=\"a\"} 2\n"));
        assert!(!out.contains("docker_container_healthy{"));
        assert!(!out.contains("docker_container_started_at_seconds{"));
    }

    #[test]
    fn escaping() {
        assert_eq!(escape("a\"b\\c\nd"), r#"a\"b\\c\nd"#);
    }
}
