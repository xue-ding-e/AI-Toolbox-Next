//! Cron expression helpers shared by the scheduled auto-update task and the
//! preview command. Uses the `croner` crate with 5-field (min hour dom mon dow)
//! expressions evaluated in the local timezone.

use std::str::FromStr;

use chrono::Local;

use croner::Cron;

/// Parse a 5-field cron expression, returning a friendly error otherwise.
pub(crate) fn parse_cron(spec: &str) -> Result<Cron, String> {
    Cron::from_str(spec.trim()).map_err(|e| format!("Invalid cron expression '{spec}': {e}"))
}

/// Compute the next `count` trigger times (strictly after now) in local time.
pub(crate) fn next_n_occurrences(spec: &str, count: usize) -> Result<Vec<String>, String> {
    let cron = parse_cron(spec)?;
    let mut next = Local::now();
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        next = cron
            .find_next_occurrence(&next, false)
            .map_err(|e| e.to_string())?;
        out.push(next.format("%Y-%m-%d %H:%M:%S").to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cron_accepts_5_fields() {
        assert!(parse_cron("0 3 * * *").is_ok());
        assert!(parse_cron("*/5 * * * *").is_ok());
    }

    #[test]
    fn parse_cron_rejects_garbage() {
        assert!(parse_cron("").is_err());
        assert!(parse_cron("not a cron").is_err());
        assert!(parse_cron("61 * * * *").is_err());
    }

    #[test]
    fn next_every_minute_is_strictly_ascending_and_future() {
        let times = next_n_occurrences("* * * * *", 10).expect("valid cron");
        assert_eq!(times.len(), 10);
        let now = Local::now();
        let parsed: Vec<_> = times
            .iter()
            .map(|s| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap())
            .collect();
        for (i, dt) in parsed.iter().enumerate() {
            assert!(
                dt > &now.naive_local(),
                "occurrence {} must be in the future",
                i
            );
            if i > 0 {
                assert!(
                    dt > &parsed[i - 1],
                    "occurrences must be strictly ascending at index {}",
                    i
                );
            }
        }
    }

    #[test]
    fn next_matches_expected_gap_for_hourly() {
        let times = next_n_occurrences("0 * * * *", 3).expect("valid cron");
        let parsed: Vec<_> = times
            .iter()
            .map(|s| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap())
            .collect();
        assert_eq!(parsed.len(), 3);
        // Consecutive top-of-hour occurrences differ by exactly 1 hour.
        for i in 1..parsed.len() {
            assert_eq!((parsed[i] - parsed[i - 1]).num_minutes(), 60);
        }
    }
}
