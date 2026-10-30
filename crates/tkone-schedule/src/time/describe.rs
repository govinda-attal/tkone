//! Plain-English description of a time spec, following the engine's rules
//! (see [`super::engine`]): the step, the phase fields finer than the driver,
//! and the window fields coarser than it.

use super::engine::{driver, fields, step, Unit};
use super::spec::{Cycle, Spec};
use crate::utils::join;

fn name(unit: Unit) -> &'static str {
    match unit {
        Unit::Second => "second",
        Unit::Minute => "minute",
        Unit::Hour => "hour",
    }
}

fn plural(n: u8, unit: Unit) -> String {
    if n == 1 {
        format!("1 {}", name(unit))
    } else {
        format!("{n} {}s", name(unit))
    }
}

/// Fields coarsest first, as they are written.
fn written(spec: &Spec) -> impl Iterator<Item = (Unit, &Cycle)> {
    fields(spec).into_iter().rev()
}

/// `"minute 30 and second 00"`, `"the start's minute and second"`,
/// `"the start's minute and second 00"`.
fn field_values<'a>(fields: impl Iterator<Item = (Unit, &'a Cycle)>) -> String {
    let fields: Vec<_> = fields.collect();
    if fields.iter().all(|(_, c)| !matches!(c, Cycle::At(_))) {
        let units: Vec<String> = fields.iter().map(|(u, _)| name(*u).to_string()).collect();
        return format!("the start's {}", join(&units));
    }
    let parts: Vec<String> = fields
        .iter()
        .map(|(u, c)| match c {
            Cycle::At(v) => format!("{} {v:02}", name(*u)),
            _ => format!("the start's {}", name(*u)),
        })
        .collect();
    join(&parts)
}

pub(crate) fn describe(spec: &Spec) -> String {
    let Some(driver) = driver(spec) else {
        // One time of day.
        let all_at: Option<Vec<u8>> = written(spec)
            .map(|(_, c)| match c {
                Cycle::At(v) => Some(*v),
                _ => None,
            })
            .collect();
        return match all_at.as_deref() {
            Some([h, m, s]) => format!("at {h:02}:{m:02}:{s:02}"),
            _ if written(spec).all(|(_, c)| *c == Cycle::AsIs) => "at the start's time of day".to_string(),
            _ => format!("at {}", field_values(written(spec))),
        };
    };

    let every: Vec<String> = written(spec)
        .filter_map(|(u, c)| match c {
            Cycle::Every(n) => Some(plural(*n, u)),
            _ => None,
        })
        .collect();
    let mut out = match every.as_slice() {
        [] => format!("every {}", name(driver)),
        [one] if one == &plural(1, driver) => format!("every {}", name(driver)),
        parts => format!("every {}", parts.join(" ")),
    };

    // Phase: fields finer than the driver.
    let finer = || written(spec).filter(|(u, _)| *u < driver);
    if finer().next().is_some() {
        let verb = if finer().any(|(_, c)| matches!(c, Cycle::At(_))) { "at" } else { "keeping" };
        out += &format!(", {verb} {}", field_values(finer()));
    }

    if step(spec, driver) != driver.duration(1) {
        out += ", counted from the start";
    }

    let pinned = |unit: Unit| {
        written(spec).find_map(|(u, c)| match c {
            Cycle::At(v) if u == unit && u > driver => Some(*v),
            _ => None,
        })
    };
    match (pinned(Unit::Hour), pinned(Unit::Minute)) {
        (Some(h), Some(m)) => out += &format!(", only during minute {h:02}:{m:02}"),
        (Some(h), None) => out += &format!(", only during the {h:02}:00 hour"),
        (None, Some(m)) => out += &format!(", only during minute {m:02} of each hour"),
        (None, None) => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_describe() {
        for (spec, expected) in [
            ("HH:MM:SS", "every second"),
            ("HH:MM:00", "every minute, at second 00"),
            ("HH:MM:_", "every minute, keeping the start's second"),
            ("HH:_:00", "every hour, at the start's minute and second 00"),
            ("1H:MM:SS", "every hour, keeping the start's minute and second"),
            ("1H:_:_", "every hour, keeping the start's minute and second"),
            ("1H:00:00", "every hour, at minute 00 and second 00"),
            ("1H:30:00", "every hour, at minute 30 and second 00"),
            ("2H:00:00", "every 2 hours, at minute 00 and second 00, counted from the start"),
            ("HH:30M:00", "every 30 minutes, at second 00, counted from the start"),
            ("1H:30M:00", "every 1 hour 30 minutes, at second 00, counted from the start"),
            ("HH:1M:00", "every minute, at second 00"),
            ("09:30M:00", "every 30 minutes, at second 00, counted from the start, only during the 09:00 hour"),
            ("09:MM:00", "every minute, at second 00, only during the 09:00 hour"),
            ("HH:30:SS", "every second, only during minute 30 of each hour"),
            ("09:30:SS", "every second, only during minute 09:30"),
            ("09:30:00", "at 09:30:00"),
            ("_:_:_", "at the start's time of day"),
            ("_:00:00", "at the start's hour, minute 00 and second 00"),
        ] {
            let parsed: Spec = spec.parse().unwrap();
            assert_eq!(describe(&parsed), expected, "{spec}");
        }
    }
}
