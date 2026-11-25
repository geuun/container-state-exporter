use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

/// 컨테이너 안에 curl이 없어서 healthcheck는 같은 바이너리로 한다.
pub fn run(port: &str) -> bool {
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
