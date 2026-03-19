use std::iter;

use super::prelude::*;
use crate::session_diagnostics;

pub(crate) struct CompartmentsParser;
impl<S: Stage> CombineAttributeParser<S> for CompartmentsParser {
    const PATH: &[Symbol] = &[sym::compartments];
    type Item = (Symbol, Span);
    const CONVERT: ConvertFn<Self::Item> = |items, span| AttributeKind::Compartments(items, span);
    const ALLOWED_TARGETS: AllowedTargets = AllowedTargets::AllowList(&[
        Allow(Target::Fn),
        Allow(Target::Static),
        Allow(Target::Struct),
        Allow(Target::Union),
        Allow(Target::Enum),
        Allow(Target::Trait),
        Allow(Target::Impl { of_trait: false }),
        Allow(Target::Impl { of_trait: true }),
        Allow(Target::Mod),
        Allow(Target::ForeignMod),
        Allow(Target::TyAlias),
        Allow(Target::MacroDef),
        Allow(Target::Const),
        Allow(Target::AssocConst),
        Allow(Target::AssocTy),
        Allow(Target::Field),
        Allow(Target::Method(MethodKind::Inherent)),
    ]);
    const TEMPLATE: AttributeTemplate = template!(List: &["c1", "c2"]);

    fn extend<'c>(
        cx: &'c mut AcceptContext<'_, '_, S>,
        args: &'c ArgParser<'_>,
    ) -> impl IntoIterator<Item = Self::Item> {
        parse_unstable(cx, args, <Self as CombineAttributeParser<S>>::PATH[0])
            .into_iter()
            .zip(iter::repeat(cx.attr_span))
    }
}

fn parse_unstable<S: Stage>(
    cx: &AcceptContext<'_, '_, S>,
    args: &ArgParser<'_>,
    symbol: Symbol,
) -> impl IntoIterator<Item = Symbol> {
    let mut res = Vec::new();

    let Some(list) = args.list() else {
        cx.emit_err(session_diagnostics::ExpectsFeatureList {
            span: cx.attr_span,
            name: symbol.to_ident_string(),
        });
        return res;
    };

    for param in list.mixed() {
        let param_span = param.span();
        if let Some(ident) = param.meta_item().and_then(|i| i.path().word()) {
            res.push(ident.name);
        } else {
            cx.emit_err(session_diagnostics::ExpectsFeatures {
                span: param_span,
                name: symbol.to_ident_string(),
            });
        }
    }

    res
}
