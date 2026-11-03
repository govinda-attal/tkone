//! The native extension behind the `tkone` Python package: `tkone._tkone`.
//!
//! One extension, with one submodule per Rust crate, so their types work
//! together (a `tkone.calendar.Calendar` can be passed to a
//! `tkone.schedule.Schedule`). The Python package re-exports each as
//! `tkone.<crate>`; see `bindings/README.md`.

mod calendar;
mod schedule;

use pyo3::prelude::*;

#[pymodule]
fn _tkone(m: &Bound<'_, PyModule>) -> PyResult<()> {
    calendar::register(m)?;
    schedule::register(m)?;
    Ok(())
}
