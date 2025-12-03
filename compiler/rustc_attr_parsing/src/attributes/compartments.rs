use std::iter;

use rustc_feature::template;
use rustc_hir::attrs::AttributeKind;
use rustc_span::{Span, Symbol, sym};
use thin_vec::ThinVec;

use super::{CombineAttributeParser, ConvertFn};
use crate::context::{AcceptContext, Stage};
use crate::parser::ArgParser;
use crate::session_diagnostics;
use crate::target_checking::{AllowedTargets, ALL_TARGETS};

pub(crate) struct CompartmentsParser;

fn convert_compartments(items: ThinVec<(Symbol, Span)>, _span: Span) -> AttributeKind {
    AttributeKind::Compartments(items)
}

impl<S: Stage> CombineAttributeParser<S> for CompartmentsParser {
    const PATH: &'static [rustc_span::Symbol] = &[sym::compartments];
    type Item = (Symbol, Span);
    const CONVERT: ConvertFn<Self::Item> = convert_compartments;
    const ALLOWED_TARGETS: AllowedTargets = AllowedTargets::AllowList(ALL_TARGETS);
    const TEMPLATE: rustc_feature::AttributeTemplate = template!(List: &["compartment_name"]);

    fn extend<'c>(
        cx: &'c mut AcceptContext<'_, '_, S>,
        args: &'c ArgParser<'_>,
    ) -> impl IntoIterator<Item = Self::Item> + 'c {
        let attr_span = cx.attr_span;
        parse_compartments(cx, args, sym::compartments).into_iter().zip(iter::repeat(attr_span))
    }
}

fn parse_compartments<'a, S: Stage>(
    cx: &mut AcceptContext<'_, '_, S>,
    args: &'a ArgParser<'a>,
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
        if let Some(meta_item) = param.meta_item() {
            if let Some(word) = meta_item.path().word() {
                if matches!(meta_item.args(), ArgParser::NoArgs) {
                    res.push(word.name);
                    continue;
                }
            }
        }
        cx.emit_err(session_diagnostics::ExpectsFeatures {
            span: param_span,
            name: symbol.to_ident_string(),
        });
    }

    res
}
