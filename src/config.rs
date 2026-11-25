//! 환경 변수 설정과 컨테이너 라벨 이름.

use std::env;

pub const EXCLUDE_LABEL: &str = "monitoring.exclude";
pub const CHANNEL_LABEL: &str = "monitoring.channel";

pub struct Config {
    /// HTTP 서버가 listen할 주소 (LISTEN_ADDR)
    pub addr: String,
    /// Docker 유닉스 소켓 경로 (DOCKER_SOCKET)
    pub socket: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            addr: env_or("LISTEN_ADDR", "0.0.0.0:9417"),
            socket: env_or("DOCKER_SOCKET", "/var/run/docker.sock"),
        }
    }

    /// listen 주소의 포트. healthcheck가 자기 자신에게 접속할 때 쓴다.
    pub fn port(&self) -> &str {
        self.addr.rsplit(':').next().unwrap_or("9417")
    }
}

fn env_or(key: &str, def: &str) -> String {
    env::var(key).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| def.to_string())
}
