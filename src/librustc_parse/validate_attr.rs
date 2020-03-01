//! Meta-syntax validation logic of attributes for post-expansion.

use crate::parse_in;

use rustc_ast::ast::{self, Attribute, MacArgs, MacDelimiter, MetaItem, MetaItemKind};
use rustc_ast::tokenstream::DelimSpan;
use rustc_errors::{Applicability, PResult};
use rustc_feature::{AttributeTemplate, BUILTIN_ATTRIBUTE_MAP};
use rustc_session::lint::builtin::ILL_FORMED_ATTRIBUTE_INPUT;
use rustc_session::parse::ParseSess;
use rustc_span::{sym, Symbol};

pub fn check_meta(sess: &ParseSess, attr: &Attribute) {
    if attr.is_doc_comment() {
        return;
    }

    let attr_info =
        attr.ident().and_then(|ident| BUILTIN_ATTRIBUTE_MAP.get(&ident.name)).map(|a| **a);

    // Check input tokens for built-in and key-value attributes.
    match attr_info {
        // `rustc_dummy` doesn't have any restrictions specific to built-in attributes.
        Some((name, _, template, _)) if name != sym::rustc_dummy => {
            check_builtin_attribute(sess, attr, name, template)
        }
        _ => {
            if let MacArgs::Eq(..) = attr.get_normal_item().args {
                // All key-value attributes are restricted to meta-item syntax.
                parse_meta(sess, attr)
                    .map_err(|mut err| {
                        err.emit();
                    })
                    .ok();
            }
        }
    }
}

pub fn parse_meta<'a>(sess: &'a ParseSess, attr: &Attribute) -> PResult<'a, MetaItem> {
    let item = attr.get_normal_item();
    Ok(MetaItem {
        span: attr.span,
        path: item.path.clone(),
        kind: match &item.args {
            MacArgs::Empty => MetaItemKind::Word,
            MacArgs::Eq(_, t) => {
                let v = parse_in(sess, t.clone(), "name value", |p| p.parse_unsuffixed_lit())?;
                MetaItemKind::NameValue(v)
            }
            MacArgs::Delimited(dspan, delim, t) => {
                check_meta_bad_delim(sess, *dspan, *delim, "wrong meta list delimiters");
                let nmis = parse_in(sess, t.clone(), "meta list", |p| p.parse_meta_seq_top())?;
                MetaItemKind::List(nmis)
            }
        },
    })
}

crate fn check_meta_bad_delim(sess: &ParseSess, span: DelimSpan, delim: MacDelimiter, msg: &str) {
    if let ast::MacDelimiter::Parenthesis = delim {
        return;
    }

    sess.span_diagnostic
        .struct_span_err(span.entire(), msg)
        .multipart_suggestion(
            "the delimiters should be `(` and `)`",
            vec![(span.open, "(".to_string()), (span.close, ")".to_string())],
            Applicability::MachineApplicable,
        )
        .emit();
}

/// Checks that the given meta-item is compatible with this `AttributeTemplate`.
fn is_attr_template_compatible(template: &AttributeTemplate, meta: &ast::MetaItemKind) -> bool {
    match meta {
        MetaItemKind::Word => template.word,
        MetaItemKind::List(..) => template.list.is_some(),
        MetaItemKind::NameValue(lit) if lit.kind.is_str() => template.name_value_str.is_some(),
        MetaItemKind::NameValue(..) => false,
    }
}

pub fn check_builtin_attribute(
    sess: &ParseSess,
    attr: &Attribute,
    name: Symbol,
    template: AttributeTemplate,
) {
    // Some special attributes like `cfg` must be checked
    // before the generic check, so we skip them here.
    let should_skip = |name| name == sym::cfg;
    // Some of previously accepted forms were used in practice,
    // report them as warnings for now.
    let should_warn = |name| {
        name == sym::doc
            || name == sym::ignore
            || name == sym::inline
            || name == sym::link
            || name == sym::test
            || name == sym::bench
    };

    match parse_meta(sess, attr) {
        Ok(meta) => {
            if !should_skip(name) && !is_attr_template_compatible(&template, &meta.kind) {
                let error_msg = format!("malformed `{}` attribute input", name);
                let mut msg = "attribute must be of the form ".to_owned();
                let mut suggestions = vec![];
                let mut first = true;
                if template.word {
                    first = false;
                    let code = format!("#[{}]", name);
                    msg.push_str(&format!("`{}`", &code));
                    suggestions.push(code);
                }
                if let Some(descr) = template.list {
                    if !first {
                        msg.push_str(" or ");
                    }
                    first = false;
                    let code = format!("#[{}({})]", name, descr);
                    msg.push_str(&format!("`{}`", &code));
                    suggestions.push(code);
                }
                if let Some(descr) = template.name_value_str {
                    if !first {
                        msg.push_str(" or ");
                    }
                    let code = format!("#[{} = \"{}\"]", name, descr);
                    msg.push_str(&format!("`{}`", &code));
                    suggestions.push(code);
                }
                if should_warn(name) {
                    sess.buffer_lint(
                        &ILL_FORMED_ATTRIBUTE_INPUT,
                        meta.span,
                        ast::CRATE_NODE_ID,
                        &msg,
                    );
                } else {
                    sess.span_diagnostic
                        .struct_span_err(meta.span, &error_msg)
                        .span_suggestions(
                            meta.span,
                            if suggestions.len() == 1 {
                                "must be of the form"
                            } else {
                                "the following are the possible correct uses"
                            },
                            suggestions.into_iter(),
                            Applicability::HasPlaceholders,
                        )
                        .emit();
                }
            }
        }
        Err(mut err) => {
            err.emit();
        }
    }
}
