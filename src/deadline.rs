use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl Deadline {
    pub fn is_expired(&self, today: &Deadline) -> bool {
        (self.year, self.month, self.day) < (today.year, today.month, today.day)
    }
}

impl fmt::Display for Deadline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl Serialize for Deadline {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Deadline {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_deadline(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid deadline: {s}")))
    }
}

/// Parse a deadline from a string.
/// Supports `YYYY-MM-DD` and `YYYY-QN` (quarter) formats.
pub fn parse_deadline(s: &str) -> Option<Deadline> {
    let s = s.trim();

    // Try YYYY-QN format first
    if let Some((year_str, q_str)) = s.split_once('-') {
        if q_str.starts_with('Q') || q_str.starts_with('q') {
            let year: u16 = year_str.parse().ok()?;
            let quarter: u8 = q_str[1..].parse().ok()?;
            if !(1..=4).contains(&quarter) {
                return None;
            }
            // Quarter end: Q1=Mar 31, Q2=Jun 30, Q3=Sep 30, Q4=Dec 31
            let (month, day) = match quarter {
                1 => (3, 31),
                2 => (6, 30),
                3 => (9, 30),
                4 => (12, 31),
                _ => unreachable!(),
            };
            return Some(Deadline { year, month, day });
        }
    }

    // Try YYYY-MM-DD format
    let parts: Vec<&str> = s.splitn(3, '-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year: u16 = parts[0].parse().ok()?;
    let month: u8 = parts[1].parse().ok()?;
    let day: u8 = parts[2].parse().ok()?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    Some(Deadline { year, month, day })
}

/// Get today's date as a `Deadline`.
pub fn today() -> Deadline {
    let now = time::OffsetDateTime::now_utc();
    Deadline {
        year: now.year() as u16,
        month: now.month() as u8,
        day: now.day(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date_format() {
        let d = parse_deadline("2025-06-01").unwrap();
        assert_eq!(d.year, 2025);
        assert_eq!(d.month, 6);
        assert_eq!(d.day, 1);
    }

    #[test]
    fn test_parse_date_with_whitespace() {
        let d = parse_deadline("  2025-06-01  ").unwrap();
        assert_eq!(d.year, 2025);
        assert_eq!(d.month, 6);
        assert_eq!(d.day, 1);
    }

    #[test]
    fn test_parse_quarter_q1() {
        let d = parse_deadline("2025-Q1").unwrap();
        assert_eq!(
            d,
            Deadline {
                year: 2025,
                month: 3,
                day: 31
            }
        );
    }

    #[test]
    fn test_parse_quarter_q2() {
        let d = parse_deadline("2025-Q2").unwrap();
        assert_eq!(
            d,
            Deadline {
                year: 2025,
                month: 6,
                day: 30
            }
        );
    }

    #[test]
    fn test_parse_quarter_q3() {
        let d = parse_deadline("2025-Q3").unwrap();
        assert_eq!(
            d,
            Deadline {
                year: 2025,
                month: 9,
                day: 30
            }
        );
    }

    #[test]
    fn test_parse_quarter_q4() {
        let d = parse_deadline("2025-Q4").unwrap();
        assert_eq!(
            d,
            Deadline {
                year: 2025,
                month: 12,
                day: 31
            }
        );
    }

    #[test]
    fn test_parse_quarter_lowercase() {
        let d = parse_deadline("2025-q2").unwrap();
        assert_eq!(
            d,
            Deadline {
                year: 2025,
                month: 6,
                day: 30
            }
        );
    }

    #[test]
    fn test_parse_invalid_quarter() {
        assert!(parse_deadline("2025-Q5").is_none());
        assert!(parse_deadline("2025-Q0").is_none());
    }

    #[test]
    fn test_parse_invalid_month() {
        assert!(parse_deadline("2025-13-01").is_none());
        assert!(parse_deadline("2025-00-01").is_none());
    }

    #[test]
    fn test_parse_invalid_day() {
        assert!(parse_deadline("2025-06-00").is_none());
        assert!(parse_deadline("2025-06-32").is_none());
    }

    #[test]
    fn test_parse_garbage() {
        assert!(parse_deadline("not-a-date").is_none());
        assert!(parse_deadline("").is_none());
        assert!(parse_deadline("alice").is_none());
    }

    #[test]
    fn test_is_expired_past_date() {
        let deadline = Deadline {
            year: 2025,
            month: 1,
            day: 1,
        };
        let today = Deadline {
            year: 2025,
            month: 6,
            day: 15,
        };
        assert!(deadline.is_expired(&today));
    }

    #[test]
    fn test_is_expired_same_date() {
        let deadline = Deadline {
            year: 2025,
            month: 6,
            day: 15,
        };
        let today = Deadline {
            year: 2025,
            month: 6,
            day: 15,
        };
        assert!(!deadline.is_expired(&today));
    }

    #[test]
    fn test_is_expired_future_date() {
        let deadline = Deadline {
            year: 2025,
            month: 12,
            day: 31,
        };
        let today = Deadline {
            year: 2025,
            month: 6,
            day: 15,
        };
        assert!(!deadline.is_expired(&today));
    }

    #[test]
    fn test_display() {
        let d = Deadline {
            year: 2025,
            month: 6,
            day: 1,
        };
        assert_eq!(d.to_string(), "2025-06-01");
    }

    #[test]
    fn test_display_padded() {
        let d = Deadline {
            year: 2025,
            month: 1,
            day: 5,
        };
        assert_eq!(d.to_string(), "2025-01-05");
    }

    #[test]
    fn test_serialize_json() {
        let d = Deadline {
            year: 2025,
            month: 6,
            day: 1,
        };
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, "\"2025-06-01\"");
    }
}
