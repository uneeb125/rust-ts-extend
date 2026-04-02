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

    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
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

    pub fn matches(&self, other: &Self) -> bool {
        if self.is_sudo() || other.is_sudo() {
            return true;
        }
        self.tags == other.tags
    }

    /// Check if `self` (current scope) can access `target` compartments.
    /// If any tag in `target` is in `trusted`, skip the check for that tag.
    /// This enables "trusted compartments" - compartments that bypass access checks.
    pub fn can_access_with_trusted(&self, target: &Self, trusted: &Self) -> bool {
        if self.is_sudo() || target.is_sudo() {
            return true;
        }
        // If any target tag is trusted, we skip the containment check for that tag
        let untrusted_target_tags: Vec<_> = target
            .tags
            .iter()
            .filter(|t| !trusted.tags.contains(t))
            .cloned()
            .collect();
        
        // All non-trusted target tags must be in current scope
        untrusted_target_tags.iter().all(|t| self.tags.contains(t))
    }
}
