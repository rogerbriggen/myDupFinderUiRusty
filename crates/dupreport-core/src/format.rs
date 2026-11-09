//! Display formatting helpers (§5.10).

const UNITS: [&str; 5] = ["KB", "MB", "GB", "TB", "PB"];

/// Human-readable size. Negative values render as an en dash.
#[must_use]
pub fn format_size(bytes: i64) -> String {
    if bytes < 0 {
        return "–".to_string();
    }
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

/// First 12 characters of a hash (whole hash when shorter, `""` when empty).
#[must_use]
pub fn format_hash_short(hash: &str) -> &str {
    match hash.char_indices().nth(12) {
        Some((idx, _)) => &hash[..idx],
        None => hash,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(format_size(-1), "–");
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(540 * 1024), "540 KB");
        assert_eq!(format_size(10 * 1024), "10 KB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(format_size(3 * 1024 * 1024 * 1024), "3.0 GB");
        assert_eq!(format_size(i64::MAX), "8192 PB");
    }

    #[test]
    fn short_hash() {
        assert_eq!(format_hash_short(""), "");
        assert_eq!(format_hash_short("ABC"), "ABC");
        assert_eq!(format_hash_short("0123456789AB"), "0123456789AB");
        assert_eq!(format_hash_short("0123456789ABCDEF"), "0123456789AB");
    }
}
