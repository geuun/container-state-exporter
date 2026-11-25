use super::{render::escape, time::parse_rfc3339, Row};
use crate::config::{CHANNEL_LABEL, EXCLUDE_LABEL};
use crate::docker::docker_json;
use std::fmt::Write as _;

/// compose가 재생성 중에 잠깐 붙이는 "<id 12자리>_이름" 컨테이너인지
fn is_transient(name: &str, id: &str) -> bool {
    name.len() > 13 && name.as_bytes()[12] == b'_' && id.starts_with(&name[..12])
}

pub fn collect(socket: &str) -> Result<Vec<Row>, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient() {
        assert!(is_transient("86986ad2f8f3_monitoring-loki-1", "86986ad2f8f38c497fc9b8be"));
        assert!(!is_transient("monitoring-loki-1", "86986ad2f8f38c497fc9b8be"));
        assert!(!is_transient("aaaaaaaaaaaa_x", "bbbbbbbbbbbbbbbb"));
    }
}
