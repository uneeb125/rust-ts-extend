use rustc_hir::def_id::DefId;
use rustc_middle::compartments::CompartmentSet;
use rustc_middle::ty::TyCtxt;
use rustc_span::Symbol;

/// Returns the trusted compartments for a given def_id.
/// This walks up the HIR tree from `def_id` to find `#[trusted_compartments(...)]`:
/// 1. First checks the item itself
/// 2. Then walks up through parent modules
/// 3. Finally checks crate-level `#![trusted_compartments(...)]`
pub(crate) fn trusted_compartments(tcx: TyCtxt<'_>, def_id: DefId) -> CompartmentSet {
    if !def_id.is_local() {
        return CompartmentSet::empty();
    }

    let local_def_id = def_id.expect_local();
    let mut hir_id = tcx.local_def_id_to_hir_id(local_def_id);

    if std::env::var("MY_DEBUG_COLLECT").is_ok() {
        println!("DEBUG: Collecting trusted compartments for {:?}", tcx.def_path_str(def_id));
    }

    // First check the starting node itself (convert HirId to OwnerId for hir_attrs)
    let start_owner_id = hir_id.owner;
    let all_attrs = tcx.hir_attrs(start_owner_id.into());
    
    if std::env::var("MY_DEBUG_COLLECT").is_ok() {
        println!(
            "DEBUG: Checking trusted compartments on {:?} ({} attrs)",
            tcx.def_path_str(def_id),
            all_attrs.len()
        );
    }

    for attr in all_attrs {
        if let rustc_hir::Attribute::Parsed(
            rustc_hir::attrs::AttributeKind::TrustedCompartments(items, _span),
        ) = attr
        {
            let trusted_tags: Vec<_> = items.iter().map(|(symbol, _)| *symbol).collect();
            if std::env::var("MY_DEBUG_COLLECT").is_ok() {
                println!(
                    "DEBUG: Found trusted compartments at {:?}: {:?}",
                    tcx.def_path_str(def_id),
                    trusted_tags
                );
            }
            return CompartmentSet::from_iter(trusted_tags);
        }
    }

    // Walk up the HIR tree through parents
    loop {
        let owner_id = tcx.hir_get_parent_item(hir_id);

        // Check if we've reached crate root (shouldn't happen after first iteration)
        if owner_id == rustc_hir::CRATE_OWNER_ID {
            if std::env::var("MY_DEBUG_COLLECT").is_ok() {
                println!("DEBUG: Reached crate root, no trusted compartments found");
            }
            return CompartmentSet::empty();
        }

        // Check this owner's attributes
        let all_attrs = tcx.hir_attrs(owner_id.into());
        
        if std::env::var("MY_DEBUG_COLLECT").is_ok() {
            println!(
                "DEBUG: Checking trusted compartments on {:?} ({} attrs)",
                tcx.def_path_str(owner_id.to_def_id()),
                all_attrs.len()
            );
        }

        for attr in all_attrs {
            if let rustc_hir::Attribute::Parsed(
                rustc_hir::attrs::AttributeKind::TrustedCompartments(items, _span),
            ) = attr
            {
                let trusted_tags: Vec<_> = items.iter().map(|(symbol, _)| *symbol).collect();
                if std::env::var("MY_DEBUG_COLLECT").is_ok() {
                    println!(
                        "DEBUG: Found trusted compartments at {:?}: {:?}",
                        tcx.def_path_str(owner_id.to_def_id()),
                        trusted_tags
                    );
                }
                return CompartmentSet::from_iter(trusted_tags);
            }
        }

        // Move to parent HirId
        hir_id = owner_id.into();
    }
}

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
