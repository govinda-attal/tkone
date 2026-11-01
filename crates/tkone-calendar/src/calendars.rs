use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::model::CalendarFile;
use crate::{Calendar, Error, Result};

/// The calendars that ship with this crate, compiled from `data/*.yaml`.
const BUILTIN: &str = include_str!("../compiled/calendars.json");

/// A set of calendars, looked up by code.
#[derive(Debug, Clone, Default)]
pub struct Calendars {
    by_code: BTreeMap<String, Calendar>,
}

impl Calendars {
    /// The calendars that ship with this crate. Built once, on first use.
    ///
    /// # Panics
    ///
    /// If the embedded data is invalid, which the crate's tests rule out.
    pub fn builtin() -> &'static Calendars {
        static BUILT: OnceLock<Calendars> = OnceLock::new();
        BUILT.get_or_init(|| Self::from_json_str(BUILTIN).expect("the built-in calendars are valid"))
    }

    /// Builds calendars from parsed files. Codes must be unique.
    pub fn from_files(files: &[CalendarFile]) -> Result<Self> {
        let mut by_code = BTreeMap::new();
        let mut problems = Vec::new();
        for file in files {
            match Calendar::from_file(file) {
                Ok(cal) => {
                    if by_code.insert(file.code.clone(), cal).is_some() {
                        return Err(Error::Invalid { code: file.code.clone(), problems: vec!["the code is used twice".into()] });
                    }
                }
                Err(e) => problems.push(e.to_string()),
            }
        }
        if !problems.is_empty() {
            return Err(Error::Parse(problems.join("\n")));
        }
        Ok(Self { by_code })
    }

    /// Builds calendars from a JSON array of calendar files, the form
    /// `compiled/calendars.json` is in.
    pub fn from_json_str(json: &str) -> Result<Self> {
        let files: Vec<CalendarFile> = serde_json::from_str(json).map_err(|e| Error::Parse(e.to_string()))?;
        Self::from_files(&files)
    }

    /// Builds calendars from every `*.yaml` file in a directory.
    #[cfg(feature = "yaml")]
    pub fn from_dir(dir: impl AsRef<std::path::Path>) -> Result<Self> {
        Self::from_files(&crate::read_dir(dir.as_ref())?)
    }

    /// The calendar with this code. Codes joined with `+` (`GBLO+USNY`) give
    /// their [joint](Calendar::joint) calendar.
    pub fn get(&self, code: &str) -> Result<Calendar> {
        let calendars = code
            .split('+')
            .map(|c| self.by_code.get(c.trim()).cloned().ok_or_else(|| Error::Unknown(c.trim().to_string())))
            .collect::<Result<Vec<_>>>()?;
        Calendar::joint(&calendars)
    }

    /// The codes, in order.
    pub fn codes(&self) -> impl Iterator<Item = &str> {
        self.by_code.keys().map(String::as_str)
    }

    /// The calendars, in code order.
    pub fn iter(&self) -> impl Iterator<Item = &Calendar> {
        self.by_code.values()
    }
}
