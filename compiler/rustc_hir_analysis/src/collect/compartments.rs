use rustc_hir::def_id::DefId;
use rustc_middle::compartments::CompartmentSet;
use rustc_middle::ty::TyCtxt;
use rustc_span::Symbol;

pub(crate) fn compartment_set(tcx: TyCtxt<'_>, def_id: DefId) -> CompartmentSet {
    if !def_id.is_local() {
        return CompartmentSet::empty();
    }

    if std::env::var("MY_DEBUG_COLLECT").is_ok() {
        println!("DEBUG: Collecting compartments for {:?}", tcx.def_path_str(def_id));
    }

    let mut raw_tags = Vec::new();

    // Get the HIR attributes directly since get_attrs might not index builtin attributes
    let local_def_id = def_id.expect_local();
    let hir_id = tcx.local_def_id_to_hir_id(local_def_id);
    let all_attrs = tcx.hir_attrs(hir_id);

    if std::env::var("MY_DEBUG_COLLECT").is_ok() {
        println!(
            "DEBUG: Found {} total attributes for {:?}",
            all_attrs.len(),
            tcx.def_path_str(def_id)
        );
    }

    for attr in all_attrs {
        if let rustc_hir::Attribute::Parsed(rustc_hir::attrs::AttributeKind::Compartments(
            items,
            _span,
        )) = attr
        {
            for (symbol, _) in items {
                raw_tags.push(*symbol);
            }
        }
    }

    if std::env::var("MY_DEBUG_COLLECT").is_ok() && !raw_tags.is_empty() {
        println!("DEBUG: Extracted compartments: {:?}", raw_tags);
    }

    // Return default compartment if none specified
    if raw_tags.is_empty() {
        // Use crate name as default when feature is active
        if tcx.features().compartments() {
            let crate_name = tcx.crate_name(def_id.krate);
            let crate_compartment = Symbol::intern(&crate_name.as_str());
            return CompartmentSet { tags: vec![crate_compartment] };
        }
        return CompartmentSet::default();
    }

    CompartmentSet::from_iter(raw_tags)
}
