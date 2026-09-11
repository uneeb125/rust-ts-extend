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

    /// Like [`Self::can_access_with_trusted`], but additionally permits access
    /// through declared `crosscomp(A-B)` pairs.
    ///
    /// A pair `A-B` is bidirectional: a target compartment `t` is accessible
    /// when `t` is one side of a declared pair whose other side appears in
    /// `self` (the source side). This lets a scope in compartment `A` reach
    /// `B` and vice versa, without granting access to unrelated compartments.
    pub fn can_access_with_crossings(
        &self,
        target: &Self,
        trusted: &Self,
        crossings: &[(Symbol, Symbol)],
    ) -> bool {
        if self.is_sudo() || target.is_sudo() {
            return true;
        }
        target.tags.iter().all(|t| {
            self.tags.contains(t)
                || trusted.tags.contains(t)
                || crossings.iter().any(|(a, b)| {
                    (a == t || b == t)
                        && (self.tags.contains(a) || self.tags.contains(b))
                })
        })
    }
}
