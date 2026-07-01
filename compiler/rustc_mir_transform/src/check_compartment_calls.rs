use rustc_index::IndexVec;
use rustc_middle::mir::interpret::Scalar;
use rustc_middle::mir::*;
use rustc_middle::ty::{Ty, TyCtxt};
use rustc_session::Session;
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

        debug!(
            "CheckCompartmentCalls: instrumented {:?} (compartment {caller_id:08X})",
            body.source.def_id()
        );
    }
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
