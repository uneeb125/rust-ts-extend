use rustc_errors::{Diag, DiagMessage, Level};
use rustc_middle::ty::TyCtxt;
use rustc_session::config::CompartmentViolation;
use rustc_span::Span;

/// Builds a diagnostic for a static compartment violation.
///
/// The severity is controlled by `-Z compartment-violations=<error|warn|allow>`:
/// - `error` (default) produces a hard error,
/// - `warn` produces a warning (upgradable via `-D warnings`),
/// - `allow` suppresses the diagnostic entirely.
///
/// Returns `None` when the violation should be suppressed.
pub(crate) fn compartment_diag(
    tcx: TyCtxt<'_>,
    span: Span,
    msg: impl Into<DiagMessage>,
) -> Option<Diag<'_, ()>> {
    let level = match tcx.sess.compartment_violations() {
        CompartmentViolation::Error => Level::Error,
        CompartmentViolation::Warn => Level::Warning,
        CompartmentViolation::Allow => return None,
    };
    Some(Diag::new(tcx.dcx(), level, msg).with_span(span))
}