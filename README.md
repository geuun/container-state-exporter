# container-state-exporter

Docker 소켓(읽기 전용)으로 컨테이너 상태를 읽어 Prometheus 메트릭으로 내보냅니다(`:9417/metrics`). 홈서버 모니터링 스택(`~/dev/infra/monitoring`)의 일부로, 그 compose가 이 디렉터리를 빌드합니다. 의존성은 `serde_json` 하나입니다.

| 메트릭 | 설명 |
|---|---|
| `container_state_running` | 1 = 실행 중 |
| `container_state_has_healthcheck` / `healthy` / `unhealthy` | healthcheck 상태 (`healthy`는 healthcheck가 있을 때만) |
| `container_state_restart_count` | Docker가 다시 시작한 횟수 |
| `container_state_oom_killed` | 마지막 종료가 OOM이면 1 |
| `container_state_exit_code` | 마지막 종료 코드 |
| `container_state_started_at_seconds` | 시작 시각 (unix) |

라벨: `container`, `compose_project`, `compose_service`, `restart_policy`, `channel`(컨테이너 라벨 `monitoring.channel`이 있을 때). `monitoring.exclude: "true"` 컨테이너와 compose가 재생성 중에 잠깐 만드는 `<id>_이름` 컨테이너는 건너뜁니다.

- `GET /healthz`: Docker `/_ping`이 되면 200
- `container-state-exporter healthcheck`: 컨테이너 healthcheck용 서브커맨드

## 개발

```sh
cargo test
cd ~/dev/infra/monitoring && docker compose up -d --build container-state-exporter
```
