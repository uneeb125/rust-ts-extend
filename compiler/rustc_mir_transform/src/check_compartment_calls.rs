use rustc_hir::def_id::DefId;
use rustc_index::IndexVec;
use rustc_middle::compartments::CompartmentSet;
use rustc_middle::mir::interpret::Scalar;
use rustc_middle::mir::*;
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_middle::ty::adjustment::PointerCoercion;
use rustc_session::Session;
use rustc_span::source_map::Spanned;
use rustc_span::Span;
use tracing::debug;

use crate::check_pointers::{
    BorrowedFieldProjectionMode, PointerCheck, check_pointers,
};

const HEADER_SIZE: i64 = 8;

pub(crate) struct CheckCompartmentCalls;

impl<'tcx> crate::MirPass<'tcx> for CheckCompartmentCalls {
    fn is_enabled(&self, sess: &Session) -> bool {
        sess.compartment_runtime_checks()
    }

    fn is_required(&self) -> bool {
        false
    }

    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        if !tcx.compartments_enabled() || !tcx.sess.compartment_runtime_checks() {
            return;
        }

        let def_id = body.source.def_id();
        let caller_set = tcx.compartment_set(def_id);

        if caller_set.is_empty() || caller_set.is_sudo() {
            return;
        }

        let caller_id = rustc_middle::ty::vtable::encode_compartment_set(&caller_set);
        let caller_const = Operand::const_from_scalar(
            tcx,
            tcx.types.u32,
            Scalar::from_u32(caller_id),
            body.span,
        );

        // [Default] means unrestricted — skip TLS to avoid overwriting
        // the caller's compartment during stdlib allocation chains.
        let default_only = caller_set.tags.len() == 1
            && caller_set.tags[0] == rustc_span::sym::Default;
        if !default_only {
            let saved_local = insert_tls_prologue(tcx, body, caller_id);
            insert_tls_epilogue(tcx, body, saved_local);
        }

        let excluded_pointees: &[Ty<'tcx>] = &[];

        check_pointers(
            tcx,
            body,
            excluded_pointees,
            |tcx, pointer, pointee_ty, _context, local_decls, stmts, source_info| {
                make_compartment_check(
                    tcx,
                    caller_const.clone(),
                    pointer,
                    pointee_ty,
                    local_decls,
                    stmts,
                    source_info,
                )
            },
            BorrowedFieldProjectionMode::NoFollowProjections,
        );

        insert_fnptr_creation_checks(tcx, body, &caller_set, caller_const.clone());

        debug!(
            "CheckCompartmentCalls: instrumented {:?} (compartment {caller_id:08X})",
            body.source.def_id()
        );
    }
}

fn insert_tls_prologue<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &mut Body<'tcx>,
    caller_id: u32,
) -> Local {
    let source_info = SourceInfo::outermost(body.span);

    let saved_local = body.local_decls.push(
        LocalDecl::with_source_info(tcx.types.u32, source_info)
    ).into();
    let saved_place = Place::from(saved_local);

    let read_fn = match tcx.lang_items().compartment_read_tls() {
        Some(def_id) => def_id,
        None => return saved_local,
    };

    let set_fn = match tcx.lang_items().compartment_set_tls() {
        Some(def_id) => def_id,
        None => return saved_local,
    };

    let basic_blocks = body.basic_blocks.as_mut();

    // Split START_BLOCK: move all statements + terminator to body_bb.
    // START_BLOCK becomes: read_tls -> read_bb
    // read_bb: set_tls -> body_bb
    // body_bb: original statements + terminator
    let body_bb = split_after_n(basic_blocks, START_BLOCK, 0);

    let read_bb = push_block(basic_blocks);
    basic_blocks[START_BLOCK].terminator = Some(call_terminator(
        tcx, read_fn, &[], saved_place, Some(read_bb), source_info, body.span,
    ));

    let caller_op = Operand::const_from_scalar(
        tcx, tcx.types.u32, Scalar::from_u32(caller_id), body.span,
    );

    let unit_local = body.local_decls.push(
        LocalDecl::with_source_info(tcx.types.unit, source_info)
    );
    let discard_place = Place::from(unit_local);
    basic_blocks[read_bb].terminator = Some(call_terminator(
        tcx, set_fn, &[Spanned { node: caller_op, span: body.span }],
        discard_place, Some(body_bb), source_info, body.span,
    ));

    saved_local
}

fn insert_tls_epilogue<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &mut Body<'tcx>,
    saved_local: Local,
) {
    let Some(set_fn) = tcx.lang_items().compartment_set_tls() else {
        return;
    };

    let source_info = SourceInfo::outermost(body.span);
    let saved_op = Operand::Copy(Place::from(saved_local));
    let unit_local = body.local_decls.push(
        LocalDecl::with_source_info(tcx.types.unit, source_info)
    );
    let discard_place = Place::from(unit_local);

    let basic_blocks = body.basic_blocks.as_mut();
    let num_blocks = basic_blocks.len();
    for bb in (0..num_blocks).rev() {
        let block = BasicBlock::from_usize(bb);
        let block_data = &basic_blocks[block];

        if block_data.is_cleanup {
            continue;
        }

        let is_exit = match block_data.terminator.as_ref().map(|t| &t.kind) {
            Some(TerminatorKind::Return) => true,
            _ => false,
        };
        if !is_exit {
            continue;
        }

        let stmts_len = basic_blocks[block].statements.len();
        let original_bb = split_after_n(basic_blocks, block, stmts_len);

        basic_blocks[block].terminator = Some(call_terminator(
            tcx, set_fn, &[Spanned { node: saved_op.clone(), span: body.span }],
            discard_place, Some(original_bb), source_info, body.span,
        ));
    }
}

fn call_terminator<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    args: &[Spanned<Operand<'tcx>>],
    destination: Place<'tcx>,
    target: Option<BasicBlock>,
    source_info: SourceInfo,
    fn_span: Span,
) -> Terminator<'tcx> {
    Terminator {
        source_info,
        kind: TerminatorKind::Call {
            func: Operand::function_handle(tcx, def_id, [], fn_span),
            args: args.into(),
            destination,
            target,
            unwind: UnwindAction::Continue,
            fn_span,
            call_source: CallSource::Misc,
        },
    }
}

fn split_after_n(
    basic_blocks: &mut IndexVec<BasicBlock, BasicBlockData<'_>>,
    block: BasicBlock,
    n: usize,
) -> BasicBlock {
    let block_data = &mut basic_blocks[block];
    let new_block = BasicBlockData::new_stmts(
        block_data.statements.split_off(n),
        block_data.terminator.take(),
        block_data.is_cleanup,
    );
    basic_blocks.push(new_block)
}

fn push_block(
    basic_blocks: &mut IndexVec<BasicBlock, BasicBlockData<'_>>,
) -> BasicBlock {
    basic_blocks.push(BasicBlockData::new(None, false))
}

fn make_compartment_check<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller_const: Operand<'tcx>,
    pointer: Place<'tcx>,
    _pointee_ty: Ty<'tcx>,
    local_decls: &mut IndexVec<Local, LocalDecl<'tcx>>,
    stmts: &mut Vec<Statement<'tcx>>,
    source_info: SourceInfo,
) -> PointerCheck<'tcx> {
    let u8_ptr_ty = Ty::new_imm_ptr(tcx, tcx.types.u8);
    let ptr_as_u8 = push_temp(
        local_decls, stmts, source_info,
        Rvalue::Cast(CastKind::PtrToPtr, Operand::Copy(pointer), u8_ptr_ty),
        u8_ptr_ty,
    );

    let neg_eight = Operand::Constant(Box::new(ConstOperand {
        span: source_info.span,
        user_ty: None,
        const_: Const::Val(
            ConstValue::Scalar(Scalar::from_int(-HEADER_SIZE, tcx.data_layout.pointer_size())),
            tcx.types.isize,
        ),
    }));
    let header_ptr = push_temp(
        local_decls, stmts, source_info,
        Rvalue::BinaryOp(
            BinOp::Offset,
            Box::new((Operand::Copy(ptr_as_u8), neg_eight)),
        ),
        u8_ptr_ty,
    );

    let u32_ptr_ty = Ty::new_imm_ptr(tcx, tcx.types.u32);
    let tag_ptr_place = push_temp(
        local_decls, stmts, source_info,
        Rvalue::Cast(CastKind::PtrToPtr, Operand::Copy(header_ptr), u32_ptr_ty),
        u32_ptr_ty,
    );
    let tag_deref = tag_ptr_place.project_deeper(&[PlaceElem::Deref], tcx);
    let tag_val = push_temp(
        local_decls, stmts, source_info,
        Rvalue::Use(Operand::Copy(tag_deref)),
        tcx.types.u32,
    );

    let cond = push_temp(
        local_decls, stmts, source_info,
        Rvalue::BinaryOp(
            BinOp::Eq,
            Box::new((caller_const, Operand::Copy(tag_val))),
        ),
        tcx.types.bool,
    );

    PointerCheck {
        cond: Operand::Copy(cond),
        assert_kind: Box::new(AssertKind::CompartmentViolation),
    }
}

fn push_temp<'tcx>(
    local_decls: &mut IndexVec<Local, LocalDecl<'tcx>>,
    stmts: &mut Vec<Statement<'tcx>>,
    source_info: SourceInfo,
    rvalue: Rvalue<'tcx>,
    ty: Ty<'tcx>,
) -> Place<'tcx> {
    let local = local_decls.push(LocalDecl::with_source_info(ty, source_info)).into();
    stmts.push(Statement::new(
        source_info,
        StatementKind::Assign(Box::new((local, rvalue))),
    ));
    local
}

fn insert_fnptr_creation_checks<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &mut Body<'tcx>,
    caller_set: &CompartmentSet,
    caller_const: Operand<'tcx>,
) {
    let basic_blocks = body.basic_blocks.as_mut();
    let local_decls = &mut body.local_decls;

    for block in basic_blocks.indices().rev() {
        for stmt_idx in (0..basic_blocks[block].statements.len()).rev() {
            let stmt = &basic_blocks[block].statements[stmt_idx];
            let source_info = stmt.source_info;

            let callee_def_id = match &stmt.kind {
                StatementKind::Assign(box (_, Rvalue::Cast(
                    CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer, _),
                    operand,
                    _,
                ))) => match operand.ty(local_decls, tcx).kind() {
                    ty::FnDef(def_id, _) => Some(*def_id),
                    _ => None,
                },
                _ => None,
            };

            let Some(callee_def_id) = callee_def_id else { continue };

            let callee_set = tcx.compartment_set(callee_def_id);
            if callee_set.is_empty() || callee_set.is_sudo() {
                continue;
            }

            let callee_id = rustc_middle::ty::vtable::encode_compartment_set(&callee_set);
            if callee_id == 0 {
                continue;
            }

            let caller_id = rustc_middle::ty::vtable::encode_compartment_set(caller_set);
            if caller_id == callee_id {
                continue;
            }

            let location = Location { block, statement_index: stmt_idx };
            let new_block = split_block(basic_blocks, location);

            let callee_const = Operand::const_from_scalar(
                tcx,
                tcx.types.u32,
                Scalar::from_u32(callee_id),
                source_info.span,
            );

            let cond = push_temp(
                local_decls,
                &mut basic_blocks[block].statements,
                source_info,
                Rvalue::BinaryOp(
                    BinOp::Eq,
                    Box::new((callee_const, caller_const.clone())),
                ),
                tcx.types.bool,
            );

            let block_data = &mut basic_blocks[block];
            block_data.terminator = Some(Terminator {
                source_info,
                kind: TerminatorKind::Assert {
                    cond: Operand::Copy(cond),
                    expected: true,
                    target: new_block,
                    msg: Box::new(AssertKind::CompartmentViolation),
                    unwind: UnwindAction::Unreachable,
                },
            });
        }
    }
}

fn split_block(
    basic_blocks: &mut IndexVec<BasicBlock, BasicBlockData<'_>>,
    location: Location,
) -> BasicBlock {
    let block_data = &mut basic_blocks[location.block];
    let new_block = BasicBlockData::new_stmts(
        block_data.statements.split_off(location.statement_index),
        block_data.terminator.take(),
        block_data.is_cleanup,
    );
    basic_blocks.push(new_block)
}
