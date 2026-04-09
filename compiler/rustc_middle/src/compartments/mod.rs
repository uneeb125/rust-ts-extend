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

    pub fn untrusted_target_compartments(&self, target: &Self, trusted: &Self) -> Vec<Symbol> {
        target
            .tags
            .iter()
            .filter(|t| !trusted.tags.contains(t) && !self.tags.contains(t))
            .cloned()
            .collect()
    }

    /// Check if `self` (current scope) can access `target` compartments.
    /// Access is allowed if all target compartments are either:
    /// 1. In `self` (current scope), OR
    /// 2. In `trusted` set (bypasses access check)
    /// 
    /// Access is DENIED only if target has compartments that are in NEITHER `self` NOR `trusted`.
    pub fn can_access_with_trusted(&self, target: &Self, trusted: &Self) -> bool {
        if self.is_sudo() || target.is_sudo() {
            return true;
        }
        // Get tags in target that are neither in self nor in trusted
        let untrusted_target_tags = self.untrusted_target_compartments(target, trusted);
        
        // Access allowed only if there are no untrusted compartments
        untrusted_target_tags.is_empty()
    }
}
