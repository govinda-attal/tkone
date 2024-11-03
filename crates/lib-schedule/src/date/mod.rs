mod iter;
mod spec;

use chrono::NaiveDateTime;
pub use iter::{NaiveSpecIterator, SpecIterator};

use spec::DayOverflow;
pub use spec::{Spec, SPEC_EXPR};

pub trait HandleOverflow {
    fn overflows(&self, overflow: DayOverflow) -> NaiveDateTime;
}
