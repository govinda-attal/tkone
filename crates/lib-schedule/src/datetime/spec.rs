use regex::Regex;
use std::str::FromStr;
use std::sync::LazyLock;

use crate::date::SPEC_EXPR as DATE_SPEC_EXPR;
use crate::prelude::*;
use crate::time::SPEC_EXPR as TIME_SPEC_EXPR;

/// # SPEC_EXPR
/// This regular expression combines the date and time spec recurrence expressions.
/// (DATE_SPEC_EXPR)T(TIME_SPEC_EXPR)
///
/// ## Examples
/// - "YY:1M:01:PT12:00:00" Recurrence on the first day of every month at 12:00:00
/// - "YY:MM:FL:PT12:00:00" Recurrence on the last Friday of every month at 12:00:00
pub static SPEC_EXPR: LazyLock<String> = LazyLock::new(|| {
    format!(
        "(?:(?<date>{})?T(?<time>{}))",
        DATE_SPEC_EXPR.as_str(),
        TIME_SPEC_EXPR
    )
    .to_string()
});
pub static SPEC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&SPEC_EXPR).unwrap());

#[derive(Debug, Clone)]
pub struct Spec {
    pub date_spec: String,
    pub time_spec: String,
}

impl FromStr for Spec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let caps = &SPEC_RE
            .captures(s)
            .ok_or(Error::ParseError("Invalid spec"))?;
        let Some(date_spec) = caps.name("date") else {
            return Err(Error::ParseError("missing date spec"));
        };
        let Some(time_spec) = caps.name("time") else {
            return Err(Error::ParseError("missing time spec"));
        };

        Ok(Self {
            date_spec: date_spec.as_str().to_string(),
            time_spec: time_spec.as_str().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_one() {
        let spec = SPEC_RE.captures("YY:1M:DD:PT12:00:00").unwrap();
        dbg!(&spec);
    }
}
