use rustc_hir::def_id::DefId;
use rustc_middle::compartments::CompartmentSet;
use rustc_middle::ty::TyCtxt;
use rustc_span::sym;

pub(crate) fn compartment_set(tcx: TyCtxt<'_>, def_id: DefId) -> CompartmentSet {
    // 1. Handle External Crates
    // The `DefId` passed to this query might come from another crate.
    // Since we aren't handling metadata decoding yet, we return an empty set for them.
    if !def_id.is_local() {
        return CompartmentSet::empty();
    }

    let mut raw_tags = Vec::new();

    // 2. Use `sym::compartments`
    // This looks up the symbol you registered in `rustc_span`.
    for attr in tcx.get_attrs(def_id, sym::compartments) {
        if let Some(list) = attr.meta_item_list() {
            for nested in list {
                if let Some(ident) = nested.ident() {
                    raw_tags.push(ident.name);
                }
            }
        }
    }

    // 3. Return Owned Value
    // The `arena_cache` modifier in the query definition generates code that
    // takes this owned value, moves it into the arena, and returns the reference.
    CompartmentSet::from_iter(raw_tags)
}
