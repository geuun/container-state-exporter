use std::fmt::Write as _;

pub fn log(level: &str, msg: &str, kv: &[(&str, String)]) {
    let mut s = format!("level={level} msg={msg:?}");
    for (k, v) in kv {
        let _ = write!(s, " {k}={v:?}");
    }
    println!("{s}");
}
