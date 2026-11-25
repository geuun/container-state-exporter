use container_state_exporter::metrics::{render::render, Row};

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
    assert!(out.contains("container_state_running{container=\"a\"} 1\n"));
    assert!(out.contains("container_state_restart_count{container=\"a\"} 2\n"));
    assert!(!out.contains("container_state_healthy{"));
    assert!(!out.contains("container_state_started_at_seconds{"));
}
