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

/// Ascending values grouped into runs of consecutive values.
fn runs(values: &[u8]) -> Vec<(u8, u8)> {
    let mut runs: Vec<(u8, u8)> = Vec::new();
    for &v in values {
        match runs.last_mut() {
            Some((_, hi)) if *hi + 1 == v => *hi = v,
            _ => runs.push((v, v)),
        }
    }
    runs
}

/// `"00 and 30"`, `"09 to 16"`, `"09 to 11 and 14"`.
fn value_list(values: &[u8]) -> String {
    let parts: Vec<String> = runs(values)
        .into_iter()
        .map(|(lo, hi)| if lo == hi { format!("{lo:02}") } else { format!("{lo:02} to {hi:02}") })
        .collect();
    join(&parts)
}

/// `"minute 30 and second 00"`, `"the start's minute and second"`,
/// `"minutes 00 and 30, second 00"`.
fn field_values<'a>(fields: impl Iterator<Item = (Unit, &'a Cycle)>) -> String {
    let fields: Vec<_> = fields.collect();
    if fields.iter().all(|(_, c)| c.pinned().is_none()) {
        let units: Vec<String> = fields.iter().map(|(u, _)| name(*u).to_string()).collect();
        return format!("the start's {}", join(&units));
    }
    let parts: Vec<String> = fields
        .iter()
        .map(|(u, c)| match c {
            Cycle::At(v) => format!("{} {v:02}", name(*u)),
            Cycle::Values(vs) => {
                format!("{}s {}", name(*u), value_list(&vs.iter().copied().collect::<Vec<_>>()))
            }
            _ => format!("the start's {}", name(*u)),
        })
        .collect();
    // A list already contains "and", so separate fields with commas.
    if fields.iter().any(|(_, c)| matches!(c, Cycle::Values(_))) {
        parts.join(", ")
    } else {
        join(&parts)
    }
}

/// `"at 09:00:00, 12:00:00 and 17:00:00"` when every field is fixed and there
/// are only a few times; otherwise field by field.
fn fixed_times(spec: &Spec) -> String {
    let pinned: Option<Vec<Vec<u8>>> = written(spec).map(|(_, c)| c.pinned()).collect();
    if let Some([hs, ms, ss]) = pinned.as_deref() {
        if hs.len() * ms.len() * ss.len() <= 8 {
            let times: Vec<String> = hs
                .iter()
                .flat_map(|h| ms.iter().flat_map(move |m| ss.iter().map(move |s| format!("{h:02}:{m:02}:{s:02}"))))
                .collect();
            return format!("at {}", join(&times));
        }
    }
    if written(spec).all(|(_, c)| *c == Cycle::AsIs) {
        return "at the start's time of day".to_string();
    }
    format!("at {}", field_values(written(spec)))
}

/// `", only during 09:00–16:59"`, `", only during minute 30 of each hour"`.
fn window(hours: Option<Vec<u8>>, minutes: Option<Vec<u8>>) -> String {
    let hour_spans = |hs: &[u8]| {
        let spans: Vec<String> =
            runs(hs).into_iter().map(|(lo, hi)| format!("{lo:02}:00–{hi:02}:59")).collect();
        join(&spans)
    };
    match (hours.as_deref(), minutes.as_deref()) {
        (Some([h]), Some([m])) => format!(", only during minute {h:02}:{m:02}"),
        (Some(hs), Some(ms)) => {
            format!(", only during {}, in minutes {} of each hour", hour_spans(hs), value_list(ms))
        }
        (Some(hs), None) => format!(", only during {}", hour_spans(hs)),
        (None, Some([m])) => format!(", only during minute {m:02} of each hour"),
        (None, Some(ms)) => format!(", only during minutes {} of each hour", value_list(ms)),
        (None, None) => String::new(),
    }
}

pub(crate) fn describe(spec: &Spec) -> String {
    let Some(driver) = driver(spec) else {
        return fixed_times(spec);
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
        let verb = if finer().any(|(_, c)| c.pinned().is_some()) { "at" } else { "keeping" };
        out += &format!(", {verb} {}", field_values(finer()));
    }

    if step(spec, driver) != driver.duration(1) {
        out += ", counted from the start";
    }

    // Window: fixed fields coarser than the driver.
    let pinned = |unit: Unit| written(spec).find(|(u, _)| *u == unit && *u > driver).and_then(|(_, c)| c.pinned());
    out + &window(pinned(Unit::Hour), pinned(Unit::Minute))
}

/// Describes a time spec used as an exclusion (after `!`) by the times it
/// covers rather than how it repeats: `12:MM:SS` is "during 12:00–12:59",
/// not "every second, only during 12:00–12:59".
pub(crate) fn describe_exclusion(spec: &Spec) -> String {
    let Some(driver) = driver(spec) else {
        return fixed_times(spec);
    };
    // Every second inside the window: the whole window is covered.
    if driver == Unit::Second && step(spec, driver) == driver.duration(1) {
        let pinned = |unit: Unit| written(spec).find(|(u, _)| *u == unit).and_then(|(_, c)| c.pinned());
        return match window(pinned(Unit::Hour), pinned(Unit::Minute)).strip_prefix(", only ") {
            Some(during) => during.to_string(),
            None => "at any time".to_string(),
        };
    }
    format!("at times matching: {}", describe(spec))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_describe_exclusion() {
        for (spec, expected) in [
            ("12:MM:SS", "during 12:00–12:59"),
            ("[12-13]:MM:SS", "during 12:00–13:59"),
            ("[12,18]:MM:SS", "during 12:00–12:59 and 18:00–18:59"),
            ("HH:[00-04]:SS", "during minutes 00 to 04 of each hour"),
            ("12:30:SS", "during minute 12:30"),
            ("HH:MM:SS", "at any time"),
            ("12:00:00", "at 12:00:00"),
            ("[12,18]:00:00", "at 12:00:00 and 18:00:00"),
            ("HH:15M:00", "at times matching: every 15 minutes, at second 00, counted from the start"),
            ("12:MM:00", "at times matching: every minute, at second 00, only during 12:00–12:59"),
        ] {
            let parsed: Spec = spec.parse().unwrap();
            assert_eq!(describe_exclusion(&parsed), expected, "{spec}");
        }
    }

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
            ("09:30M:00", "every 30 minutes, at second 00, counted from the start, only during 09:00–09:59"),
            ("09:MM:00", "every minute, at second 00, only during 09:00–09:59"),
            ("09-16:15M:00", "every 15 minutes, at second 00, counted from the start, only during 09:00–16:59"),
            ("[09-11,14]:30M:00", "every 30 minutes, at second 00, counted from the start, only during 09:00–11:59 and 14:00–14:59"),
            ("HH:[00,30]:00", "every hour, at minutes 00 and 30, second 00"),
            ("HH:00-14:SS", "every second, only during minutes 00 to 14 of each hour"),
            ("[09,10]:[00,30]:SS", "every second, only during 09:00–10:59, in minutes 00 and 30 of each hour"),
            ("[09,12,17]:00:00", "at 09:00:00, 12:00:00 and 17:00:00"),
            ("[09,17]:[00,30]:00", "at 09:00:00, 09:30:00, 17:00:00 and 17:30:00"),
            ("09-16:[00,30]:00", "at hours 09 to 16, minutes 00 and 30, second 00"),
            ("[09,12]:_:00", "at hours 09 and 12, the start's minute, second 00"),
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
