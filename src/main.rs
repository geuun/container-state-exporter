use container_state_exporter::{config::Config, server};

fn main() {
    let cfg = Config::from_env();
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        std::process::exit(if server::healthcheck::run(cfg.port()) { 0 } else { 1 });
    }
    server::serve(&cfg);
}
