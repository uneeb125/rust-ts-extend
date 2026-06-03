use rustc_hir::def_id::DefId;
use rustc_middle::compartments::CompartmentSet;
use rustc_middle::ty::TyCtxt;
use rustc_session::config::CompartmentMissing;
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

    // First check if this is an associated item in an impl block - check impl's trusted_compartments
    if let Some(impl_def_id) = tcx.impl_of_assoc(def_id) {
        if let Some(local_impl_id) = impl_def_id.as_local() {
            let impl_hir_id = tcx.local_def_id_to_hir_id(local_impl_id);
            let impl_owner_id = impl_hir_id.owner;
            let impl_attrs = tcx.hir_attrs(impl_owner_id.into());
            
            for attr in impl_attrs {
                if let rustc_hir::Attribute::Parsed(
                    rustc_hir::attrs::AttributeKind::TrustedCompartments(items, _span),
                ) = attr
                {
                    let trusted_tags: Vec<_> = items.iter().map(|(symbol, _)| *symbol).collect();
                    if std::env::var("MY_DEBUG_COLLECT").is_ok() {
                        println!(
                            "DEBUG: Found trusted compartments on impl block {:?}: {:?}",
                            tcx.def_path_str(impl_def_id),
                            trusted_tags
                        );
                    }
                    return CompartmentSet::from_iter(trusted_tags);
                }
            }
        }
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

    if let Some(trusted) = get_partition_trusted(tcx, def_id) {
        return trusted;
    }

    // Walk up the HIR tree through parents
    loop {
        let owner_id = tcx.hir_get_parent_item(hir_id);

        // Check if we've reached crate root
        if owner_id == rustc_hir::CRATE_OWNER_ID {
            // Check crate-level attributes for #[trusted_compartments(...)]
            // Crate root attributes are stored with CRATE_OWNER_ID
            let crate_attrs = tcx.hir_attrs(rustc_hir::CRATE_OWNER_ID.into());
            
            if std::env::var("MY_DEBUG_COLLECT").is_ok() {
                println!(
                    "DEBUG: Checking crate-level attributes for trusted compartments ({} attrs)",
                    crate_attrs.len()
                );
            }
            
            for attr in crate_attrs {
                if let rustc_hir::Attribute::Parsed(
                    rustc_hir::attrs::AttributeKind::TrustedCompartments(items, _span),
                ) = attr
                {
                    let trusted_tags: Vec<_> = items.iter().map(|(symbol, _)| *symbol).collect();
                    if std::env::var("MY_DEBUG_COLLECT").is_ok() {
                        println!(
                            "DEBUG: Found trusted compartments at crate level: {:?}",
                            trusted_tags
                        );
                    }
                    return CompartmentSet::from_iter(trusted_tags);
                }
            }
            
            if std::env::var("MY_DEBUG_COLLECT").is_ok() {
                println!("DEBUG: No trusted compartments found at crate level");
            }
            
            // For #[automatically_derived] code (e.g., derive macros), inherit trusted compartments from self type
            if let Some(impl_def_id) = tcx.impl_of_assoc(def_id) {
                if tcx.is_automatically_derived(impl_def_id) {
                    if let Some(local_impl_id) = impl_def_id.as_local() {
                        let impl_hir_id = tcx.local_def_id_to_hir_id(local_impl_id);
                        if let rustc_hir::Node::Item(rustc_hir::Item {
                            kind: rustc_hir::ItemKind::Impl(impl_block),
                            ..
                        }) = tcx.hir_node(impl_hir_id) {
                            if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = impl_block.self_ty.kind {
                                if let rustc_hir::def::Res::Def(def_kind, adt_def_id) = path.res {
                                    if matches!(def_kind, rustc_hir::def::DefKind::Struct | rustc_hir::def::DefKind::Enum | rustc_hir::def::DefKind::Union) {
                                        if std::env::var("MY_DEBUG_COLLECT").is_ok() {
                                            println!("DEBUG: #[automatically_derived] code, inheriting trusted compartments from self type {:?}", adt_def_id);
                                        }
                                        return trusted_compartments(tcx, adt_def_id);
                                    }
                                }
                            }
                        }
                    }
                }
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

fn get_partition_trusted(tcx: TyCtxt<'_>, def_id: DefId) -> Option<CompartmentSet> {
    let Some(ref partition_map) = tcx.sess.compartment_partition_map else {
        return None;
    };
    let def_kind = tcx.def_kind(def_id);
    if def_kind != rustc_hir::def::DefKind::Fn && def_kind != rustc_hir::def::DefKind::AssocFn {
        return None;
    }
    if tcx.generics_of(def_id).requires_monomorphization(tcx) {
        return None;
    }
    let crate_name = tcx.crate_name(def_id.krate);
    let def_path = rustc_middle::ty::print::with_no_trimmed_paths!(tcx.def_path_str(def_id));
    let full_path = format!("{}::{}", crate_name, def_path);
    partition_map.get(full_path.as_str()).and_then(|entry| {
        if entry.trusted.is_empty() {
            None
        } else {
            Some(CompartmentSet::from_iter(entry.trusted.iter().cloned()))
        }
    })
}

fn get_partition_compartments(tcx: TyCtxt<'_>, def_id: DefId) -> Option<CompartmentSet> {
    let Some(ref partition_map) = tcx.sess.compartment_partition_map else {
        return None;
    };
    let def_kind = tcx.def_kind(def_id);
    if def_kind != rustc_hir::def::DefKind::Fn && def_kind != rustc_hir::def::DefKind::AssocFn {
        return None;
    }
    if tcx.generics_of(def_id).requires_monomorphization(tcx) {
        return None;
    }
    let crate_name = tcx.crate_name(def_id.krate);
    let def_path = rustc_middle::ty::print::with_no_trimmed_paths!(tcx.def_path_str(def_id));
    let full_path = format!("{}::{}", crate_name, def_path);
    match partition_map.get(full_path.as_str()) {
        Some(entry) => Some(CompartmentSet::from_iter(
            entry.compartments.iter().cloned(),
        )),
        None => {
            match tcx.sess.opts.unstable_opts.compartment_missing {
                CompartmentMissing::Skip => {}
                CompartmentMissing::Error => {
                    tcx.dcx().span_err(
                        tcx.def_span(def_id),
                        format!(
                            "function `{full_path}` not found in compartment partition file"
                        ),
                    );
                }
                CompartmentMissing::Warn => {
                    tcx.dcx().span_warn(
                        tcx.def_span(def_id),
                        format!(
                            "function `{full_path}` not found in compartment partition file, \
                             using default compartments"
                        ),
                    );
                }
            }
            None
        }
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
        // For trait impl methods or derive-generated impl methods, check the self type's compartments
        if let Some(self_type_compartments) = get_self_type_compartments(tcx, def_id) {
            if std::env::var("MY_DEBUG_COLLECT").is_ok() {
                println!("DEBUG: Using self type compartments for {:?}: {:?}", def_id, self_type_compartments.tags);
            }
            return self_type_compartments;
        }

        if let Some(partition_compartments) = get_partition_compartments(tcx, def_id) {
            if std::env::var("MY_DEBUG_COLLECT").is_ok() {
                println!("DEBUG: Using partition compartments for {:?}: {:?}", def_id, partition_compartments.tags);
            }
            return partition_compartments;
        }

        // Use crate name as default when feature is active
        if tcx.compartments_enabled() {
            let crate_name = tcx.crate_name(def_id.krate);
            let crate_compartment = Symbol::intern(&crate_name.as_str());
            return CompartmentSet { tags: vec![crate_compartment] };
        }
        return CompartmentSet::default();
    }

    CompartmentSet::from_iter(raw_tags)
}

/// For trait impl methods or derive-generated impl methods, get the compartments from the self type
fn get_self_type_compartments(tcx: TyCtxt<'_>, def_id: DefId) -> Option<CompartmentSet> {
    if std::env::var("MY_DEBUG_COLLECT").is_ok() {
        println!("DEBUG: get_self_type_compartments called for {:?}", tcx.def_path_str(def_id));
    }
    
    // First check if this is an associated item in an impl or trait
    if let Some(impl_def_id) = tcx.impl_of_assoc(def_id) {
        if std::env::var("MY_DEBUG_COLLECT").is_ok() {
            println!("DEBUG: impl_of_assoc returned {:?}", tcx.def_path_str(impl_def_id));
        }
        // Get the self type from the impl block
        let local_impl_id = impl_def_id.as_local()?;
        let impl_hir_id = tcx.local_def_id_to_hir_id(local_impl_id);
        
        // Look at the impl block to find self_ty
        let impl_item = tcx.hir_node(impl_hir_id);
        if let rustc_hir::Node::Item(rustc_hir::Item {
            kind: rustc_hir::ItemKind::Impl(impl_block),
            ..
        }) = impl_item {
            let self_ty = impl_block.self_ty;
            if std::env::var("MY_DEBUG_COLLECT").is_ok() {
                println!("DEBUG: impl self_ty kind = {:?}", self_ty.kind);
            }
            if let rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, path)) = self_ty.kind {
                if let rustc_hir::def::Res::Def(def_kind, adt_def_id) = path.res {
                    if matches!(def_kind, rustc_hir::def::DefKind::Struct | rustc_hir::def::DefKind::Enum | rustc_hir::def::DefKind::Union) {
                        if std::env::var("MY_DEBUG_COLLECT").is_ok() {
                            println!("DEBUG: Found ADT def_id = {:?}", tcx.def_path_str(adt_def_id));
                        }
                        // Recursively get compartments for the self type
                        let adt_compartments = compartment_set(tcx, adt_def_id);
                        if std::env::var("MY_DEBUG_COLLECT").is_ok() {
                            println!("DEBUG: ADT compartments = {:?}", adt_compartments.tags);
                        }
                        // Only return if the self type has explicit compartments (not just crate default)
                        if !adt_compartments.tags.is_empty() {
                            let crate_name = tcx.crate_name(def_id.krate);
                            let is_crate_default = adt_compartments.tags.len() == 1 && 
                                adt_compartments.tags[0].as_str() == crate_name.as_str();
                            if !is_crate_default {
                                if std::env::var("MY_DEBUG_COLLECT").is_ok() {
                                    println!("DEBUG: Returning ADT compartments");
                                }
                                return Some(adt_compartments);
                            }
                        }
                    }
                }
            }
        }
    } else {
        if std::env::var("MY_DEBUG_COLLECT").is_ok() {
            println!("DEBUG: impl_of_assoc returned None for {:?}", tcx.def_path_str(def_id));
        }
    }
    
    // Check if this is a variant constructor (Ctor(Variant, ...))
    // Variant constructors are not impl items, so impl_of_assoc returns None
    // But we can check if the parent item is an enum/struct/union
    if def_id.is_local() {
        let local_def_id = def_id.expect_local();
        let hir_id = tcx.local_def_id_to_hir_id(local_def_id);
        let parent_item = tcx.hir_get_parent_item(hir_id);
        
        // Check if parent is an enum/struct/union
        let parent_def_kind = tcx.def_kind(parent_item.to_def_id());
        if matches!(parent_def_kind, rustc_hir::def::DefKind::Enum | rustc_hir::def::DefKind::Struct | rustc_hir::def::DefKind::Union) {
            let adt_compartments = compartment_set(tcx, parent_item.to_def_id());
            if !adt_compartments.tags.is_empty() {
                let crate_name = tcx.crate_name(def_id.krate);
                let is_crate_default = adt_compartments.tags.len() == 1 && 
                    adt_compartments.tags[0].as_str() == crate_name.as_str();
                if !is_crate_default {
                    return Some(adt_compartments);
                }
            }
        }
    }
    
    None
}
