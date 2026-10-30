//! Union specs: `A | B` yields the occurrences of every part, in order.
//!
//! Each part runs as its own iterator; [`Merge`] interleaves their results by
//! actual time. Results must have strictly increasing actual times, as for a
//! single spec, so when two parts produce the same actual time only the first
//! listed part's occurrence is kept.

use fallible_iterator::FallibleIterator;

/// The parts of a union spec, or `None` for a plain spec.
pub(crate) fn union_parts(spec: &str) -> Option<Vec<&str>> {
    spec.contains('|').then(|| spec.split('|').map(str::trim).collect())
}

/// Interleaves several iterators by `key` (an occurrence's actual time),
/// dropping results whose key is not after the last one yielded.
#[derive(Debug, Clone)]
pub(crate) struct Merge<I: FallibleIterator, K> {
    parts: Vec<I>,
    /// Each part's next result; empty until iteration starts.
    heads: Vec<Option<I::Item>>,
    key: fn(&I::Item) -> K,
    last: Option<K>,
}

impl<I, K> Merge<I, K>
where
    I: FallibleIterator,
    K: Ord + Clone,
{
    pub(crate) fn new(parts: Vec<I>, key: fn(&I::Item) -> K) -> Self {
        Self { parts, heads: Vec::new(), key, last: None }
    }

    /// The parts, for applying settings before iteration starts.
    pub(crate) fn parts_mut(&mut self) -> &mut [I] {
        &mut self.parts
    }

    pub(crate) fn next(&mut self) -> Result<Option<I::Item>, I::Error> {
        if self.heads.is_empty() {
            for part in &mut self.parts {
                self.heads.push(part.next()?);
            }
        }
        loop {
            // `min_by_key` keeps the first of equal keys: the first listed part.
            let Some(i) = (0..self.heads.len())
                .filter(|&i| self.heads[i].is_some())
                .min_by_key(|&i| (self.key)(self.heads[i].as_ref().unwrap()))
            else {
                return Ok(None);
            };
            let item = self.heads[i].take().unwrap();
            self.heads[i] = self.parts[i].next()?;
            let key = (self.key)(&item);
            if self.last.as_ref().is_some_and(|last| key <= *last) {
                continue;
            }
            self.last = Some(key);
            return Ok(Some(item));
        }
    }
}
