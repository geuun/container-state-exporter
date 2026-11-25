//! container-state-exporter: Docker 컨테이너 상태(실행 여부, healthcheck, 재시작 횟수, OOM)를
//! Prometheus 형식으로 노출한다. cAdvisor는 healthcheck 상태를 주지 않아서 따로 둔다.
//! Docker API는 GET만 호출한다.
//!
//! 컨테이너 라벨
//!   monitoring.exclude=true   메트릭에서 뺀다
//!   monitoring.channel=<이름>  channel 라벨로 붙인다 (알림 채널 라우팅)
//!
//! 사용: `container-state-exporter` (서버) / `container-state-exporter healthcheck` (컨테이너 healthcheck용)

pub mod config;
pub mod docker;
pub mod log;
pub mod metrics;
pub mod server;
