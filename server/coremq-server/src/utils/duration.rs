use std::time::Duration;

use serde::{Deserialize, Deserializer};

/*
  Parse a human-readable duration such as "500ms", "3s", "5m" or "1h".
  A bare number is interpreted as seconds.
*/
pub fn parse_duration(raw: &str) -> Result<Duration, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("empty duration".to_string());
    }

    let (digits, suffix) = match s.find(|c: char| !c.is_ascii_digit()) {
        Some(idx) => s.split_at(idx),
        None => (s, ""),
    };

    if digits.is_empty() {
        return Err(format!("duration '{}' has no leading number", raw));
    }

    let value: u64 = digits
        .parse()
        .map_err(|_| format!("duration '{}' has an invalid number", raw))?;

    match suffix.trim() {
        "" | "s" => Ok(Duration::from_secs(value)),
        "ms" => Ok(Duration::from_millis(value)),
        "m" => Ok(Duration::from_secs(value * 60)),
        "h" => Ok(Duration::from_secs(value * 3600)),
        other => Err(format!(
            "duration '{}' has unknown unit '{}' (expected ms, s, m or h)",
            raw, other
        )),
    }
}

/*
  Serde adapter so config files can spell durations as "10s" instead of a raw
  number of seconds. Accepts both a string and a bare integer (seconds).
*/
pub fn de_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Text(String),
        Secs(u64),
    }

    match Raw::deserialize(deserializer)? {
        Raw::Text(s) => parse_duration(&s).map_err(serde::de::Error::custom),
        Raw::Secs(n) => Ok(Duration::from_secs(n)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_units() {
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_duration("3s").unwrap(), Duration::from_secs(3));
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("42").unwrap(), Duration::from_secs(42));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("s").is_err());
        assert!(parse_duration("10x").is_err());
    }
}
