use rustc_middle::mir::*;
use rustc_middle::ty::TyCtxt;
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

        let mut call_count = 0;
        for (bb, data) in body.basic_blocks.iter().enumerate() {
            let Some(ref terminator) = data.terminator else { continue; };
            if let TerminatorKind::Call { .. } = &terminator.kind {
                call_count += 1;
                debug!("CheckCompartmentCalls: call in bb{bb}");
            }
        }
        debug!(
            "CheckCompartmentCalls: {call_count} call(s) in {:?}",
            body.source.def_id()
        );
    }
}
