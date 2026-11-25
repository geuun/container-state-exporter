use super::Row;
use std::fmt::Write as _;

/// Prometheus 라벨 값 이스케이프: \ " 줄바꿈
pub fn escape(s: &str) -> String {
    s.replace('\\', r"\\").replace('"', "\\\"").replace('\n', r"\n")
}

pub fn render(rows: &[Row]) -> String {
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
    metric("container_state_running", "gauge", "1 if the container is running.", &|r| Some(r.running as i64));
    metric("container_state_has_healthcheck", "gauge", "1 if the container defines a healthcheck.", &|r| {
        Some(has_hc(r) as i64)
    });
    metric(
        "container_state_healthy",
        "gauge",
        "1 healthy, 0 otherwise. Only for containers with a healthcheck.",
        &|r| has_hc(r).then(|| (r.health == "healthy") as i64),
    );
    metric("container_state_unhealthy", "gauge", "1 if the healthcheck reports unhealthy.", &|r| {
        Some((r.health == "unhealthy") as i64)
    });
    metric("container_state_restart_count", "counter", "Restarts done by the Docker restart policy.", &|r| {
        Some(r.restart_count)
    });
    metric("container_state_oom_killed", "gauge", "1 if the last exit was an OOM kill.", &|r| {
        Some(r.oom_killed as i64)
    });
    metric("container_state_exit_code", "gauge", "Exit code of the last run.", &|r| Some(r.exit_code));
    metric("container_state_started_at_seconds", "gauge", "Unix time the container last started.", &|r| {
        r.started_at
    });
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping() {
        assert_eq!(escape("a\"b\\c\nd"), r#"a\"b\\c\nd"#);
    }
}
