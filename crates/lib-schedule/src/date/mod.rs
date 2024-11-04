mod iter;
mod spec;

pub use iter::{NaiveSpecIterator, SpecIterator};

pub use spec::DayOverflow;
pub use spec::{Spec, SPEC_EXPR};

// pub trait HandleOverflow {
//     fn overflows(&self, overflow: DayOverflow) -> NaiveDateTime;
// }
