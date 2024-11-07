// use std::str::FromStr;

// use once_cell::sync::Lazy;
// use regex::Regex;
// use crate::date::SPEC_EXPR as DATE_SPEC_EXPR;
// use crate::time::SPEC_EXPR as TIME_SPEC_EXPR;
// use crate::prelude::*;

// pub static SPEC_EXPR: Lazy<String> =
//     Lazy::new(|| format!("(?:(?<date>{DATE_SPEC_EXPR})?T(?<time>{TIME_SPEC_EXPR}))").to_string());
// pub static SPEC_RE: Lazy<Regex> = Lazy::new(|| Regex::new(&SPEC_EXPR).unwrap());

// #[derive(Debug, Clone)]
// pub struct Spec {
//     pub date_spec: String,
//     pub time_spec: String,
// }


// impl FromStr for Spec {
//     type Err = Error;

//     fn from_str(s: &str) -> Result<Self> {
//         let caps = &SPEC_RE
//             .captures(s)
//             .ok_or(Error::ParseError("Invalid spec"))?;
//         let Some(date_spec) = caps.name("date") else {
//             return Err(Error::ParseError("missing date spec"));
//         };
//         let Some(time_spec) = caps.name("time") else {
//             return Err(Error::ParseError("missing time spec"));
//         };

//         Ok(Self {
//             date_spec: date_spec.as_str().to_string(),
//             time_spec: time_spec.as_str().to_string(),
//         })
//     }
// }


// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_one() {
//         let spec = SPEC_RE.captures("YY:1M:DD:PT12:00:00").unwrap();
//         dbg!(&spec);
//     }
// }
