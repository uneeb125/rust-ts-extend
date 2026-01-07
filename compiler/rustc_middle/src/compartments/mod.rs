use rustc_macros::HashStable;
use rustc_span::Symbol;

#[derive(Clone, Debug, PartialEq, Eq, Hash, HashStable)]
pub struct CompartmentSet {
    pub tags: Vec<Symbol>,
}

impl CompartmentSet {
    pub fn empty() -> Self {
        Self { tags: Vec::new() }
    }

    pub fn from_iter<I: IntoIterator<Item = Symbol>>(iter: I) -> Self {
        let mut tags: Vec<Symbol> = iter.into_iter().collect();
        tags.sort_unstable();
        tags.dedup();
        Self { tags }
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        self.tags.iter().all(|tag| other.contains(*tag))
    }

    pub fn contains(&self, tag: Symbol) -> bool {
        self.tags.binary_search(&tag).is_ok()
    }
}
