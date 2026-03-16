use rustc_macros::{HashStable, TyDecodable, TyEncodable, TypeFoldable, TypeVisitable};
use rustc_span::{Symbol,sym};

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    HashStable,
    TyEncodable,
    TyDecodable,
    TypeFoldable,
    TypeVisitable
)]
pub struct CompartmentSet {
    pub tags: Vec<Symbol>,
}

impl CompartmentSet {
    pub fn empty() -> Self {
        Self { tags: Vec::new() }
    }

    pub fn default() -> Self {
        Self { tags: vec![sym::Default] }
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
        if self.is_sudo() || target.is_sudo() {
            return true;
        }
        target.tags.iter().all(|t| self.tags.contains(t))
    }
}
