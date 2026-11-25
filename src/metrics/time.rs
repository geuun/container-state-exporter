/// Docker의 RFC3339 시각(나노초 포함 가능)을 Unix 초로. 0001-01-01(시작한 적 없음)이면 None.
pub fn parse_rfc3339(s: &str) -> Option<i64> {
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (num(0..4)?, num(5..7)?, num(8..10)?, num(11..13)?, num(14..16)?, num(17..19)?);
    if y < 2000 {
        return None;
    }
    // 시간대: Z 또는 ±hh:mm (소수 초는 건너뛴다)
    let rest = &s[19..];
    let tz = rest.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let offset = match tz.as_bytes().first() {
        Some(b'Z') | None => 0,
        Some(&sign) => {
            let oh: i64 = tz.get(1..3)?.parse().ok()?;
            let om: i64 = tz.get(4..6)?.parse().ok()?;
            let o = oh * 3600 + om * 60;
            if sign == b'-' { -o } else { o }
        }
    };
    // Howard Hinnant의 days_from_civil
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + se - offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339() {
        assert_eq!(parse_rfc3339("2026-11-20T02:51:41.329482Z"), Some(1_795_143_101));
        assert_eq!(parse_rfc3339("2026-11-20T11:51:41+09:00"), Some(1_795_143_101));
        assert_eq!(parse_rfc3339("0001-01-01T00:00:00Z"), None);
        assert_eq!(parse_rfc3339(""), None);
    }
}
