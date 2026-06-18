use rustc_index::IndexVec;
use rustc_middle::mir::interpret::Scalar;
use rustc_middle::mir::*;
use rustc_middle::ty::{self, Instance, TyCtxt};
use rustc_session::Session;
use tracing::debug;

pub(crate) struct CheckCompartmentCalls;

impl<'tcx> crate::MirPass<'tcx> for CheckCompartmentCalls {
    fn is_enabled(&self, sess: &Session) -> bool {
        sess.compartment_runtime_checks()
    }

    fn is_required(&self) -> bool {
        false
    }

    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        if !tcx.compartments_enabled() {
            return;
        }

        let caller_set = tcx.compartment_set(body.source.def_id());
        if caller_set.is_empty() {
            return;
        }

        let typing_env = body.typing_env(tcx);
        let basic_blocks = body.basic_blocks.as_mut();
        let local_decls = &mut body.local_decls;

        // Iterate backwards to avoid index invalidation after block splits
        for bb in basic_blocks.indices().rev() {
            let Some(ref terminator) = basic_blocks[bb].terminator else { continue; };
            let TerminatorKind::Call {
                func: Operand::Constant(box ConstOperand { const_, .. }),
                ..
            } = &terminator.kind
            else {
                continue;
            };

            let ty::FnDef(def_id, args) = *const_.ty().kind() else { continue; };

            let Ok(Some(instance)) = Instance::try_resolve(tcx, typing_env, def_id, args)
            else {
                continue;
            };

            let ty::InstanceKind::Virtual(trait_def_id, vtable_index) = instance.def else {
                continue;
            };

            debug!(
                "CheckCompartmentCalls: virtual call in bb{bb:?} trait={trait_def_id:?} idx={vtable_index}"
            );

            let source_info = terminator.source_info;

            // Split block: put the call terminator into a new block
            let new_bb = split_block(basic_blocks, Location {
                block: bb,
                statement_index: basic_blocks[bb].statements.len(),
            });

            // Inject a placeholder true assert for v1.
            // v2 will load callee compartment from vtable and compare with caller_id.
            let block_data = &mut basic_blocks[bb];
            let ok_local = local_decls
                .push(LocalDecl::with_source_info(tcx.types.bool, source_info))
                .into();

            let const_true = Operand::Constant(Box::new(ConstOperand {
                span: source_info.span,
                user_ty: None,
                const_: Const::Val(
                    ConstValue::Scalar(Scalar::from_bool(true)),
                    tcx.types.bool,
                ),
            }));

            block_data.statements.push(Statement::new(
                source_info,
                StatementKind::Assign(Box::new((ok_local, Rvalue::Use(const_true)))),
            ));

            block_data.terminator = Some(Terminator {
                source_info,
                kind: TerminatorKind::Assert {
                    cond: Operand::Copy(ok_local),
                    expected: true,
                    target: new_bb,
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
