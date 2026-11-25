//! HTTP 서버: /metrics, /healthz.

pub mod healthcheck;

use crate::config::Config;
use crate::docker::docker_get;
use crate::log::log;
use crate::metrics::{collect::collect, render::render};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

/// 서버를 띄우고 요청을 처리한다. bind에 실패하면 프로세스를 종료한다.
pub fn serve(cfg: &Config) -> ! {
    let listener = TcpListener::bind(&cfg.addr).unwrap_or_else(|e| {
        log("ERROR", "bind failed", &[("addr", cfg.addr.clone()), ("err", e.to_string())]);
        std::process::exit(1);
    });
    let started = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    log(
        "INFO",
        "listening",
        &[("addr", cfg.addr.clone()), ("socket", cfg.socket.clone()), ("started", started.to_string())],
    );
    // 요청은 15초마다 한 번(Alloy) + healthcheck 정도라 순서대로 처리한다.
    for conn in listener.incoming().flatten() {
        handle(conn, &cfg.socket);
    }
    std::process::exit(0)
}
