//! 컨테이너 상태 수집(`collect`)과 Prometheus 텍스트 변환(`render`).

pub mod collect;
pub mod render;
pub mod time;

/// 컨테이너 한 개의 상태 스냅샷.
pub struct Row {
    pub labels: String,
    pub running: bool,
    /// "healthy" | "unhealthy" | "starting" | "none"
    pub health: String,
    pub restart_count: i64,
    pub oom_killed: bool,
    pub exit_code: i64,
    pub started_at: Option<i64>,
}
