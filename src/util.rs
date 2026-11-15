//! 通用工具：时间格式化（UTC+8 显示）与字符串处理。
//! 这些函数无状态、不依赖 UI 与存储，单独成模块便于复用与测试。

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 把「自 1970-01-01 起的天数」换算成公历年月日（Howard Hinnant 算法）。
pub fn ymd(secs_utc8: u64) -> (i64, u32, u32) {
    let z = (secs_utc8 / 86400) as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 简短时间：今天显示「HH:MM」，今年显示「MM-DD HH:MM」，跨年显示「YYYY-MM-DD」。
pub fn fmt_time(secs: u64) -> String {
    let s = secs + 8 * 3600;
    let now = now_secs() + 8 * 3600;
    let (y, m, d) = ymd(s);
    let (ny, nm, nd) = ymd(now);
    let hh = (s % 86400) / 3600;
    let mm = (s % 3600) / 60;
    if (y, m, d) == (ny, nm, nd) {
        format!("{hh:02}:{mm:02}")
    } else if y == ny {
        format!("{m:02}-{d:02} {hh:02}:{mm:02}")
    } else {
        format!("{y}-{m:02}-{d:02}")
    }
}

/// 相对时间：刚刚 / N 分钟前 / N 小时前 / 昨天 / N 天前 / N 个月前。
pub fn fmt_ago(secs: u64) -> String {
    let diff = now_secs().saturating_sub(secs);
    if diff < 60 {
        "刚刚".into()
    } else if diff < 3600 {
        format!("{} 分钟前", diff / 60)
    } else if diff < 86400 {
        format!("{} 小时前", diff / 3600)
    } else if diff < 172_800 {
        "昨天".into()
    } else if diff < 86400 * 30 {
        format!("{} 天前", diff / 86400)
    } else {
        format!("{} 个月前", diff / (86400 * 30))
    }
}

/// 按「字符」截断，超出部分用省略号，避免中文被按字节切断。
pub fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// 取内容的第一行作为列表摘要。
#[allow(dead_code)]
pub fn first_line(s: &str) -> String {
    let t = s.trim();
    match t.find('\n') {
        Some(i) => t[..i].trim().to_owned(),
        None => t.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_handles_cjk() {
        assert_eq!(truncate("一二三四五", 3), "一二…");
        assert_eq!(truncate("一二", 3), "一二");
    }

    #[test]
    fn first_line_trims() {
        assert_eq!(first_line("  第一行\n第二行  "), "第一行");
    }

    #[test]
    fn ymd_known_date() {
        assert_eq!(ymd(0), (1970, 1, 1));
    }
}
