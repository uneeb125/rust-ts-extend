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

    pub fn is_sudo(&self) -> bool {
        self.tags.iter().any(|s| s.as_str() == "sudo")
    }

    pub fn can_access(&self, target: &Self) -> bool {
        if self.is_sudo() {
            return true;
        }
        target.tags.iter().all(|t| self.tags.contains(t))
    }
}
