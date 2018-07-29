// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::borrow::Cow;
use std::cmp::min;

use config::lists::*;
use syntax::codemap::{BytePos, CodeMap, Span};
use syntax::parse::token::DelimToken;
use syntax::{ast, ptr};

use chains::rewrite_chain;
use closures;
use codemap::{LineRangeUtils, SpanUtils};
use comment::{
    combine_strs_with_missing_comments, contains_comment, recover_comment_removed, rewrite_comment,
    rewrite_missing_comment, CharClasses, FindUncommented,
};
use config::{Config, ControlBraceStyle, IndentStyle};
use lists::{
    definitive_tactic, itemize_list, shape_for_tactic, struct_lit_formatting, struct_lit_shape,
    struct_lit_tactic, write_list, ListFormatting, ListItem, Separator,
};
use macros::{rewrite_macro, MacroArg, MacroPosition};
use matches::rewrite_match;
use overflow;
use pairs::{rewrite_all_pairs, rewrite_pair, PairParts};
use patterns::{can_be_overflowed_pat, is_short_pattern, TuplePatField};
use rewrite::{Rewrite, RewriteContext};
use shape::{Indent, Shape};
use spanned::Spanned;
use string::{rewrite_string, StringFormat};
use types::{can_be_overflowed_type, rewrite_path, PathContext};
use utils::{
    colon_spaces, contains_skip, count_newlines, first_line_ends_with, first_line_width,
    inner_attributes, last_line_extendable, last_line_width, mk_sp, outer_attributes,
    ptr_vec_to_ref_vec, semicolon_for_stmt, wrap_str,
};
use vertical::rewrite_with_alignment;
use visitor::FmtVisitor;

impl Rewrite for ast::Expr {
    fn rewrite(&self, context: &RewriteContext, shape: Shape) -> Option<String> {
        format_expr(self, ExprType::SubExpression, context, shape)
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum ExprType {
    Statement,
    SubExpression,
}

pub fn format_expr(
    expr: &ast::Expr,
    expr_type: ExprType,
    context: &RewriteContext,
    shape: Shape,
) -> Option<String> {
    skip_out_of_file_lines_range!(context, expr.span);

    if contains_skip(&*expr.attrs) {
        return Some(context.snippet(expr.span()).to_owned());
    }

    let expr_rw = match expr.node {
        ast::ExprKind::Array(ref expr_vec) => rewrite_array(
            "",
            &ptr_vec_to_ref_vec(expr_vec),
            expr.span,
            context,
            shape,
            choose_separator_tactic(context, expr.span),
            None,
        ),
        ast::ExprKind::Lit(ref l) => rewrite_literal(context, l, shape),
        ast::ExprKind::Call(ref callee, ref args) => {
            let inner_span = mk_sp(callee.span.hi(), expr.span.hi());
            let callee_str = callee.rewrite(context, shape)?;
            rewrite_call(context, &callee_str, args, inner_span, shape)
        }
        ast::ExprKind::Paren(ref subexpr) => rewrite_paren(context, subexpr, shape, expr.span),
        ast::ExprKind::Binary(op, ref lhs, ref rhs) => {
            // FIXME: format comments between operands and operator
            rewrite_all_pairs(expr, shape, context).or_else(|| {
                rewrite_pair(
                    &**lhs,
                    &**rhs,
                    PairParts::infix(&format!(" {} ", context.snippet(op.span))),
                    context,
                    shape,
                    context.config.binop_separator(),
                )
            })
        }
        ast::ExprKind::Unary(ref op, ref subexpr) => rewrite_unary_op(context, op, subexpr, shape),
        ast::ExprKind::Struct(ref path, ref fields, ref base) => rewrite_struct_lit(
            context,
            path,
            fields,
            base.as_ref().map(|e| &**e),
            expr.span,
            shape,
        ),
        ast::ExprKind::Tup(ref items) => {
            rewrite_tuple(context, &ptr_vec_to_ref_vec(items), expr.span, shape)
        }
        ast::ExprKind::If(..)
        | ast::ExprKind::IfLet(..)
        | ast::ExprKind::ForLoop(..)
        | ast::ExprKind::Loop(..)
        | ast::ExprKind::While(..)
        | ast::ExprKind::WhileLet(..) => to_control_flow(expr, expr_type)
            .and_then(|control_flow| control_flow.rewrite(context, shape)),
        ast::ExprKind::Block(ref block, opt_label) => {
            match expr_type {
                ExprType::Statement => {
                    if is_unsafe_block(block) {
                        rewrite_block(block, Some(&expr.attrs), opt_label, context, shape)
                    } else if let rw @ Some(_) =
                        rewrite_empty_block(context, block, Some(&expr.attrs), opt_label, "", shape)
                    {
                        // Rewrite block without trying to put it in a single line.
                        rw
                    } else {
                        let prefix = block_prefix(context, block, shape)?;

                        rewrite_block_with_visitor(
                            context,
                            &prefix,
                            block,
                            Some(&expr.attrs),
                            opt_label,
                            shape,
                            true,
                        )
                    }
                }
                ExprType::SubExpression => {
                    rewrite_block(block, Some(&expr.attrs), opt_label, context, shape)
                }
            }
        }
        ast::ExprKind::Match(ref cond, ref arms) => {
            rewrite_match(context, cond, arms, shape, expr.span, &expr.attrs)
        }
        ast::ExprKind::Path(ref qself, ref path) => {
            rewrite_path(context, PathContext::Expr, qself.as_ref(), path, shape)
        }
        ast::ExprKind::Assign(ref lhs, ref rhs) => {
            rewrite_assignment(context, lhs, rhs, None, shape)
        }
        ast::ExprKind::AssignOp(ref op, ref lhs, ref rhs) => {
            rewrite_assignment(context, lhs, rhs, Some(op), shape)
        }
        ast::ExprKind::Continue(ref opt_label) => {
            let id_str = match *opt_label {
                Some(label) => format!(" {}", label.ident),
                None => String::new(),
            };
            Some(format!("continue{}", id_str))
        }
        ast::ExprKind::Break(ref opt_label, ref opt_expr) => {
            let id_str = match *opt_label {
                Some(label) => format!(" {}", label.ident),
                None => String::new(),
            };

            if let Some(ref expr) = *opt_expr {
                rewrite_unary_prefix(context, &format!("break{} ", id_str), &**expr, shape)
            } else {
                Some(format!("break{}", id_str))
            }
        }
        ast::ExprKind::Yield(ref opt_expr) => if let Some(ref expr) = *opt_expr {
            rewrite_unary_prefix(context, "yield ", &**expr, shape)
        } else {
            Some("yield".to_string())
        },
        ast::ExprKind::Closure(capture, asyncness, movability, ref fn_decl, ref body, _) => {
            closures::rewrite_closure(
                capture, asyncness, movability, fn_decl, body, expr.span, context, shape,
            )
        }
        ast::ExprKind::Try(..) | ast::ExprKind::Field(..) | ast::ExprKind::MethodCall(..) => {
            rewrite_chain(expr, context, shape)
        }
        ast::ExprKind::Mac(ref mac) => {
            rewrite_macro(mac, None, context, shape, MacroPosition::Expression).or_else(|| {
                wrap_str(
                    context.snippet(expr.span).to_owned(),
                    context.config.max_width(),
                    shape,
                )
            })
        }
        ast::ExprKind::Ret(None) => Some("return".to_owned()),
        ast::ExprKind::Ret(Some(ref expr)) => {
            rewrite_unary_prefix(context, "return ", &**expr, shape)
        }
        ast::ExprKind::Box(ref expr) => rewrite_unary_prefix(context, "box ", &**expr, shape),
        ast::ExprKind::AddrOf(mutability, ref expr) => {
            rewrite_expr_addrof(context, mutability, expr, shape)
        }
        ast::ExprKind::Cast(ref expr, ref ty) => rewrite_pair(
            &**expr,
            &**ty,
            PairParts::infix(" as "),
            context,
            shape,
            SeparatorPlace::Front,
        ),
        ast::ExprKind::Type(ref expr, ref ty) => rewrite_pair(
            &**expr,
            &**ty,
            PairParts::infix(": "),
            context,
            shape,
            SeparatorPlace::Back,
        ),
        ast::ExprKind::Index(ref expr, ref index) => {
            rewrite_index(&**expr, &**index, context, shape)
        }
        ast::ExprKind::Repeat(ref expr, ref repeats) => rewrite_pair(
            &**expr,
            &*repeats.value,
            PairParts::new("[", "; ", "]"),
            context,
            shape,
            SeparatorPlace::Back,
        ),
        ast::ExprKind::Range(ref lhs, ref rhs, limits) => {
            let delim = match limits {
                ast::RangeLimits::HalfOpen => "..",
                ast::RangeLimits::Closed => "..=",
            };

            fn needs_space_before_range(context: &RewriteContext, lhs: &ast::Expr) -> bool {
                match lhs.node {
                    ast::ExprKind::Lit(ref lit) => match lit.node {
                        ast::LitKind::FloatUnsuffixed(..) => {
                            context.snippet(lit.span).ends_with('.')
                        }
                        _ => false,
                    },
                    _ => false,
                }
            }

            fn needs_space_after_range(rhs: &ast::Expr) -> bool {
                match rhs.node {
                    // Don't format `.. ..` into `....`, which is invalid.
                    //
                    // This check is unnecessary for `lhs`, because a range
                    // starting from another range needs parentheses as `(x ..) ..`
                    // (`x .. ..` is a range from `x` to `..`).
                    ast::ExprKind::Range(None, _, _) => true,
                    _ => false,
                }
            }

            let default_sp_delim = |lhs: Option<&ast::Expr>, rhs: Option<&ast::Expr>| {
                let space_if = |b: bool| if b { " " } else { "" };

                format!(
                    "{}{}{}",
                    lhs.map(|lhs| space_if(needs_space_before_range(context, lhs)))
                        .unwrap_or(""),
                    delim,
                    rhs.map(|rhs| space_if(needs_space_after_range(rhs)))
                        .unwrap_or(""),
                )
            };

            match (lhs.as_ref().map(|x| &**x), rhs.as_ref().map(|x| &**x)) {
                (Some(lhs), Some(rhs)) => {
                    let sp_delim = if context.config.spaces_around_ranges() {
                        format!(" {} ", delim)
                    } else {
                        default_sp_delim(Some(lhs), Some(rhs))
                    };
                    rewrite_pair(
                        &*lhs,
                        &*rhs,
                        PairParts::infix(&sp_delim),
                        context,
                        shape,
                        context.config.binop_separator(),
                    )
                }
                (None, Some(rhs)) => {
                    let sp_delim = if context.config.spaces_around_ranges() {
                        format!("{} ", delim)
                    } else {
                        default_sp_delim(None, Some(rhs))
                    };
                    rewrite_unary_prefix(context, &sp_delim, &*rhs, shape)
                }
                (Some(lhs), None) => {
                    let sp_delim = if context.config.spaces_around_ranges() {
                        format!(" {}", delim)
                    } else {
                        default_sp_delim(Some(lhs), None)
                    };
                    rewrite_unary_suffix(context, &sp_delim, &*lhs, shape)
                }
                (None, None) => Some(delim.to_owned()),
            }
        }
        // We do not format these expressions yet, but they should still
        // satisfy our width restrictions.
        ast::ExprKind::InlineAsm(..) => Some(context.snippet(expr.span).to_owned()),
        ast::ExprKind::Catch(ref block) => {
            if let rw @ Some(_) = rewrite_single_line_block(
                context,
                "do catch ",
                block,
                Some(&expr.attrs),
                None,
                shape,
            ) {
                rw
            } else {
                // 9 = `do catch `
                let budget = shape.width.saturating_sub(9);
                Some(format!(
                    "{}{}",
                    "do catch ",
                    rewrite_block(
                        block,
                        Some(&expr.attrs),
                        None,
                        context,
                        Shape::legacy(budget, shape.indent)
                    )?
                ))
            }
        }
        // FIXME(#2743)
        ast::ExprKind::ObsoleteInPlace(..) => unimplemented!(),
        ast::ExprKind::Async(capture_by, _node_id, ref block) => {
            let mover = if capture_by == ast::CaptureBy::Value {
                "move "
            } else {
                ""
            };
            if let rw @ Some(_) = rewrite_single_line_block(
                context,
                format!("{}{}", "async ", mover).as_str(),
                block,
                Some(&expr.attrs),
                None,
                shape,
            ) {
                rw
            } else {
                // 6 = `async `
                let budget = shape.width.saturating_sub(6);
                Some(format!(
                    "{}{}{}",
                    "async ",
                    mover,
                    rewrite_block(
                        block,
                        Some(&expr.attrs),
                        None,
                        context,
                        Shape::legacy(budget, shape.indent)
                    )?
                ))
            }
        }
    };

    expr_rw
        .and_then(|expr_str| recover_comment_removed(expr_str, expr.span, context))
        .and_then(|expr_str| {
            let attrs = outer_attributes(&expr.attrs);
            let attrs_str = attrs.rewrite(context, shape)?;
            let span = mk_sp(
                attrs.last().map_or(expr.span.lo(), |attr| attr.span.hi()),
                expr.span.lo(),
            );
            combine_strs_with_missing_comments(context, &attrs_str, &expr_str, span, shape, false)
        })
}

pub fn rewrite_array<T: Rewrite + Spanned + ToExpr>(
    name: &str,
    exprs: &[&T],
    span: Span,
    context: &RewriteContext,
    shape: Shape,
    force_separator_tactic: Option<SeparatorTactic>,
    delim_token: Option<DelimToken>,
) -> Option<String> {
    overflow::rewrite_with_square_brackets(
        context,
        name,
        exprs,
        shape,
        span,
        force_separator_tactic,
        delim_token,
    )
}

fn rewrite_empty_block(
    context: &RewriteContext,
    block: &ast::Block,
    attrs: Option<&[ast::Attribute]>,
    label: Option<ast::Label>,
    prefix: &str,
    shape: Shape,
) -> Option<String> {
    let label_str = rewrite_label(label);
    if attrs.map_or(false, |a| !inner_attributes(a).is_empty()) {
        return None;
    }

    if block.stmts.is_empty() && !block_contains_comment(block, context.codemap) && shape.width >= 2
    {
        return Some(format!("{}{}{{}}", prefix, label_str));
    }

    // If a block contains only a single-line comment, then leave it on one line.
    let user_str = context.snippet(block.span);
    let user_str = user_str.trim();
    if user_str.starts_with('{') && user_str.ends_with('}') {
        let comment_str = user_str[1..user_str.len() - 1].trim();
        if block.stmts.is_empty()
            && !comment_str.contains('\n')
            && !comment_str.starts_with("//")
            && comment_str.len() + 4 <= shape.width
        {
            return Some(format!("{}{}{{ {} }}", prefix, label_str, comment_str));
        }
    }

    None
}

fn block_prefix(context: &RewriteContext, block: &ast::Block, shape: Shape) -> Option<String> {
    Some(match block.rules {
        ast::BlockCheckMode::Unsafe(..) => {
            let snippet = context.snippet(block.span);
            let open_pos = snippet.find_uncommented("{")?;
            // Extract comment between unsafe and block start.
            let trimmed = &snippet[6..open_pos].trim();

            if !trimmed.is_empty() {
                // 9 = "unsafe  {".len(), 7 = "unsafe ".len()
                let budget = shape.width.checked_sub(9)?;
                format!(
                    "unsafe {} ",
                    rewrite_comment(
                        trimmed,
                        true,
                        Shape::legacy(budget, shape.indent + 7),
                        context.config,
                    )?
                )
            } else {
                "unsafe ".to_owned()
            }
        }
        ast::BlockCheckMode::Default => String::new(),
    })
}

fn rewrite_single_line_block(
    context: &RewriteContext,
    prefix: &str,
    block: &ast::Block,
    attrs: Option<&[ast::Attribute]>,
    label: Option<ast::Label>,
    shape: Shape,
) -> Option<String> {
    if is_simple_block(block, attrs, context.codemap) {
        let expr_shape = shape.offset_left(last_line_width(prefix))?;
        let expr_str = block.stmts[0].rewrite(context, expr_shape)?;
        let label_str = rewrite_label(label);
        let result = format!("{}{}{{ {} }}", prefix, label_str, expr_str);
        if result.len() <= shape.width && !result.contains('\n') {
            return Some(result);
        }
    }
    None
}

pub fn rewrite_block_with_visitor(
    context: &RewriteContext,
    prefix: &str,
    block: &ast::Block,
    attrs: Option<&[ast::Attribute]>,
    label: Option<ast::Label>,
    shape: Shape,
    has_braces: bool,
) -> Option<String> {
    if let rw @ Some(_) = rewrite_empty_block(context, block, attrs, label, prefix, shape) {
        return rw;
    }

    let mut visitor = FmtVisitor::from_context(context);
    visitor.block_indent = shape.indent;
    visitor.is_if_else_block = context.is_if_else_block();
    match block.rules {
        ast::BlockCheckMode::Unsafe(..) => {
            let snippet = context.snippet(block.span);
            let open_pos = snippet.find_uncommented("{")?;
            visitor.last_pos = block.span.lo() + BytePos(open_pos as u32)
        }
        ast::BlockCheckMode::Default => visitor.last_pos = block.span.lo(),
    }

    let inner_attrs = attrs.map(inner_attributes);
    let label_str = rewrite_label(label);
    visitor.visit_block(block, inner_attrs.as_ref().map(|a| &**a), has_braces);
    Some(format!("{}{}{}", prefix, label_str, visitor.buffer))
}

impl Rewrite for ast::Block {
    fn rewrite(&self, context: &RewriteContext, shape: Shape) -> Option<String> {
        rewrite_block(self, None, None, context, shape)
    }
}

fn rewrite_block(
    block: &ast::Block,
    attrs: Option<&[ast::Attribute]>,
    label: Option<ast::Label>,
    context: &RewriteContext,
    shape: Shape,
) -> Option<String> {
    let prefix = block_prefix(context, block, shape)?;

    // shape.width is used only for the single line case: either the empty block `{}`,
    // or an unsafe expression `unsafe { e }`.
    if let rw @ Some(_) = rewrite_empty_block(context, block, attrs, label, &prefix, shape) {
        return rw;
    }

    let result = rewrite_block_with_visitor(context, &prefix, block, attrs, label, shape, true);
    if let Some(ref result_str) = result {
        if result_str.lines().count() <= 3 {
            if let rw @ Some(_) =
                rewrite_single_line_block(context, &prefix, block, attrs, label, shape)
            {
                return rw;
            }
        }
    }

    result
}

impl Rewrite for ast::Stmt {
    fn rewrite(&self, context: &RewriteContext, shape: Shape) -> Option<String> {
        skip_out_of_file_lines_range!(context, self.span());

        let result = match self.node {
            ast::StmtKind::Local(ref local) => local.rewrite(context, shape),
            ast::StmtKind::Expr(ref ex) | ast::StmtKind::Semi(ref ex) => {
                let suffix = if semicolon_for_stmt(context, self) {
                    ";"
                } else {
                    ""
                };

                let shape = shape.sub_width(suffix.len())?;
                format_expr(ex, ExprType::Statement, context, shape).map(|s| s + suffix)
            }
            ast::StmtKind::Mac(..) | ast::StmtKind::Item(..) => None,
        };
        result.and_then(|res| recover_comment_removed(res, self.span(), context))
    }
}

// Rewrite condition if the given expression has one.
pub fn rewrite_cond(context: &RewriteContext, expr: &ast::Expr, shape: Shape) -> Option<String> {
    match expr.node {
        ast::ExprKind::Match(ref cond, _) => {
            // `match `cond` {`
            let cond_shape = match context.config.indent_style() {
                IndentStyle::Visual => shape.shrink_left(6).and_then(|s| s.sub_width(2))?,
                IndentStyle::Block => shape.offset_left(8)?,
            };
            cond.rewrite(context, cond_shape)
        }
        _ => to_control_flow(expr, ExprType::SubExpression).and_then(|control_flow| {
            let alt_block_sep =
                String::from("\n") + &shape.indent.block_only().to_string(context.config);
            control_flow
                .rewrite_cond(context, shape, &alt_block_sep)
                .and_then(|rw| Some(rw.0))
        }),
    }
}

// Abstraction over control flow expressions
#[derive(Debug)]
struct ControlFlow<'a> {
    cond: Option<&'a ast::Expr>,
    block: &'a ast::Block,
    else_block: Option<&'a ast::Expr>,
    label: Option<ast::Label>,
    pats: Vec<&'a ast::Pat>,
    keyword: &'a str,
    matcher: &'a str,
    connector: &'a str,
    allow_single_line: bool,
    // True if this is an `if` expression in an `else if` :-( hacky
    nested_if: bool,
    span: Span,
}

fn to_control_flow(expr: &ast::Expr, expr_type: ExprType) -> Option<ControlFlow> {
    match expr.node {
        ast::ExprKind::If(ref cond, ref if_block, ref else_block) => Some(ControlFlow::new_if(
            cond,
            vec![],
            if_block,
            else_block.as_ref().map(|e| &**e),
            expr_type == ExprType::SubExpression,
            false,
            expr.span,
        )),
        ast::ExprKind::IfLet(ref pat, ref cond, ref if_block, ref else_block) => {
            Some(ControlFlow::new_if(
                cond,
                ptr_vec_to_ref_vec(pat),
                if_block,
                else_block.as_ref().map(|e| &**e),
                expr_type == ExprType::SubExpression,
                false,
                expr.span,
            ))
        }
        ast::ExprKind::ForLoop(ref pat, ref cond, ref block, label) => {
            Some(ControlFlow::new_for(pat, cond, block, label, expr.span))
        }
        ast::ExprKind::Loop(ref block, label) => {
            Some(ControlFlow::new_loop(block, label, expr.span))
        }
        ast::ExprKind::While(ref cond, ref block, label) => Some(ControlFlow::new_while(
            vec![],
            cond,
            block,
            label,
            expr.span,
        )),
        ast::ExprKind::WhileLet(ref pat, ref cond, ref block, label) => Some(
            ControlFlow::new_while(ptr_vec_to_ref_vec(pat), cond, block, label, expr.span),
        ),
        _ => None,
    }
}

fn choose_matcher(pats: &[&ast::Pat]) -> &'static str {
    if pats.is_empty() {
        ""
    } else {
        "let"
    }
}

impl<'a> ControlFlow<'a> {
    fn new_if(
        cond: &'a ast::Expr,
        pats: Vec<&'a ast::Pat>,
        block: &'a ast::Block,
        else_block: Option<&'a ast::Expr>,
        allow_single_line: bool,
        nested_if: bool,
        span: Span,
    ) -> ControlFlow<'a> {
        let matcher = choose_matcher(&pats);
        ControlFlow {
            cond: Some(cond),
            block,
            else_block,
            label: None,
            pats,
            keyword: "if",
            matcher,
            connector: " =",
            allow_single_line,
            nested_if,
            span,
        }
    }

    fn new_loop(block: &'a ast::Block, label: Option<ast::Label>, span: Span) -> ControlFlow<'a> {
        ControlFlow {
            cond: None,
            block,
            else_block: None,
            label,
            pats: vec![],
            keyword: "loop",
            matcher: "",
            connector: "",
            allow_single_line: false,
            nested_if: false,
            span,
        }
    }

    fn new_while(
        pats: Vec<&'a ast::Pat>,
        cond: &'a ast::Expr,
        block: &'a ast::Block,
        label: Option<ast::Label>,
        span: Span,
    ) -> ControlFlow<'a> {
        let matcher = choose_matcher(&pats);
        ControlFlow {
            cond: Some(cond),
            block,
            else_block: None,
            label,
            pats,
            keyword: "while",
            matcher,
            connector: " =",
            allow_single_line: false,
            nested_if: false,
            span,
        }
    }

    fn new_for(
        pat: &'a ast::Pat,
        cond: &'a ast::Expr,
        block: &'a ast::Block,
        label: Option<ast::Label>,
        span: Span,
    ) -> ControlFlow<'a> {
        ControlFlow {
            cond: Some(cond),
            block,
            else_block: None,
            label,
            pats: vec![pat],
            keyword: "for",
            matcher: "",
            connector: " in",
            allow_single_line: false,
            nested_if: false,
            span,
        }
    }

    fn rewrite_single_line(
        &self,
        pat_expr_str: &str,
        context: &RewriteContext,
        width: usize,
    ) -> Option<String> {
        assert!(self.allow_single_line);
        let else_block = self.else_block?;
        let fixed_cost = self.keyword.len() + "  {  } else {  }".len();

        if let ast::ExprKind::Block(ref else_node, _) = else_block.node {
            if !is_simple_block(self.block, None, context.codemap)
                || !is_simple_block(else_node, None, context.codemap)
                || pat_expr_str.contains('\n')
            {
                return None;
            }

            let new_width = width.checked_sub(pat_expr_str.len() + fixed_cost)?;
            let expr = &self.block.stmts[0];
            let if_str = expr.rewrite(context, Shape::legacy(new_width, Indent::empty()))?;

            let new_width = new_width.checked_sub(if_str.len())?;
            let else_expr = &else_node.stmts[0];
            let else_str = else_expr.rewrite(context, Shape::legacy(new_width, Indent::empty()))?;

            if if_str.contains('\n') || else_str.contains('\n') {
                return None;
            }

            let result = format!(
                "{} {} {{ {} }} else {{ {} }}",
                self.keyword, pat_expr_str, if_str, else_str
            );

            if result.len() <= width {
                return Some(result);
            }
        }

        None
    }
}

impl<'a> ControlFlow<'a> {
    fn rewrite_pat_expr(
        &self,
        context: &RewriteContext,
        expr: &ast::Expr,
        shape: Shape,
        offset: usize,
    ) -> Option<String> {
        debug!("rewrite_pat_expr {:?} {:?} {:?}", shape, self.pats, expr);

        let cond_shape = shape.offset_left(offset)?;
        if !self.pats.is_empty() {
            let matcher = if self.matcher.is_empty() {
                self.matcher.to_owned()
            } else {
                format!("{} ", self.matcher)
            };
            let pat_shape = cond_shape
                .offset_left(matcher.len())?
                .sub_width(self.connector.len())?;
            let pat_string = rewrite_multiple_patterns(context, &self.pats, pat_shape)?;
            let result = format!("{}{}{}", matcher, pat_string, self.connector);
            return rewrite_assign_rhs(context, result, expr, cond_shape);
        }

        let expr_rw = expr.rewrite(context, cond_shape);
        // The expression may (partially) fit on the current line.
        // We do not allow splitting between `if` and condition.
        if self.keyword == "if" || expr_rw.is_some() {
            return expr_rw;
        }

        // The expression won't fit on the current line, jump to next.
        let nested_shape = shape
            .block_indent(context.config.tab_spaces())
            .with_max_width(context.config);
        let nested_indent_str = nested_shape.indent.to_string_with_newline(context.config);
        expr.rewrite(context, nested_shape)
            .map(|expr_rw| format!("{}{}", nested_indent_str, expr_rw))
    }

    fn rewrite_cond(
        &self,
        context: &RewriteContext,
        shape: Shape,
        alt_block_sep: &str,
    ) -> Option<(String, usize)> {
        // Do not take the rhs overhead from the upper expressions into account
        // when rewriting pattern.
        let new_width = context.budget(shape.used_width());
        let fresh_shape = Shape {
            width: new_width,
            ..shape
        };
        let constr_shape = if self.nested_if {
            // We are part of an if-elseif-else chain. Our constraints are tightened.
            // 7 = "} else " .len()
            fresh_shape.offset_left(7)?
        } else {
            fresh_shape
        };

        let label_string = rewrite_label(self.label);
        // 1 = space after keyword.
        let offset = self.keyword.len() + label_string.len() + 1;

        let pat_expr_string = match self.cond {
            Some(cond) => self.rewrite_pat_expr(context, cond, constr_shape, offset)?,
            None => String::new(),
        };

        let brace_overhead =
            if context.config.control_brace_style() != ControlBraceStyle::AlwaysNextLine {
                // 2 = ` {`
                2
            } else {
                0
            };
        let one_line_budget = context
            .config
            .max_width()
            .saturating_sub(constr_shape.used_width() + offset + brace_overhead);
        let force_newline_brace = (pat_expr_string.contains('\n')
            || pat_expr_string.len() > one_line_budget)
            && !last_line_extendable(&pat_expr_string);

        // Try to format if-else on single line.
        if self.allow_single_line
            && context
                .config
                .width_heuristics()
                .single_line_if_else_max_width
                > 0
        {
            let trial = self.rewrite_single_line(&pat_expr_string, context, shape.width);

            if let Some(cond_str) = trial {
                if cond_str.len() <= context
                    .config
                    .width_heuristics()
                    .single_line_if_else_max_width
                {
                    return Some((cond_str, 0));
                }
            }
        }

        let cond_span = if let Some(cond) = self.cond {
            cond.span
        } else {
            mk_sp(self.block.span.lo(), self.block.span.lo())
        };

        // `for event in event`
        // Do not include label in the span.
        let lo = self
            .label
            .map_or(self.span.lo(), |label| label.ident.span.hi());
        let between_kwd_cond = mk_sp(
            context
                .snippet_provider
                .span_after(mk_sp(lo, self.span.hi()), self.keyword.trim()),
            if self.pats.is_empty() {
                cond_span.lo()
            } else if self.matcher.is_empty() {
                self.pats[0].span.lo()
            } else {
                context
                    .snippet_provider
                    .span_before(self.span, self.matcher.trim())
            },
        );

        let between_kwd_cond_comment = extract_comment(between_kwd_cond, context, shape);

        let after_cond_comment =
            extract_comment(mk_sp(cond_span.hi(), self.block.span.lo()), context, shape);

        let block_sep = if self.cond.is_none() && between_kwd_cond_comment.is_some() {
            ""
        } else if context.config.control_brace_style() == ControlBraceStyle::AlwaysNextLine
            || force_newline_brace
        {
            alt_block_sep
        } else {
            " "
        };

        let used_width = if pat_expr_string.contains('\n') {
            last_line_width(&pat_expr_string)
        } else {
            // 2 = spaces after keyword and condition.
            label_string.len() + self.keyword.len() + pat_expr_string.len() + 2
        };

        Some((
            format!(
                "{}{}{}{}{}",
                label_string,
                self.keyword,
                between_kwd_cond_comment.as_ref().map_or(
                    if pat_expr_string.is_empty() || pat_expr_string.starts_with('\n') {
                        ""
                    } else {
                        " "
                    },
                    |s| &**s,
                ),
                pat_expr_string,
                after_cond_comment.as_ref().map_or(block_sep, |s| &**s)
            ),
            used_width,
        ))
    }
}

impl<'a> Rewrite for ControlFlow<'a> {
    fn rewrite(&self, context: &RewriteContext, shape: Shape) -> Option<String> {
        debug!("ControlFlow::rewrite {:?} {:?}", self, shape);

        let alt_block_sep = &shape.indent.to_string_with_newline(context.config);
        let (cond_str, used_width) = self.rewrite_cond(context, shape, alt_block_sep)?;
        // If `used_width` is 0, it indicates that whole control flow is written in a single line.
        if used_width == 0 {
            return Some(cond_str);
        }

        let block_width = shape.width.saturating_sub(used_width);
        // This is used only for the empty block case: `{}`. So, we use 1 if we know
        // we should avoid the single line case.
        let block_width = if self.else_block.is_some() || self.nested_if {
            min(1, block_width)
        } else {
            block_width
        };
        let block_shape = Shape {
            width: block_width,
            ..shape
        };
        let block_str = {
            let old_val = context.is_if_else_block.replace(self.else_block.is_some());
            let result =
                rewrite_block_with_visitor(context, "", self.block, None, None, block_shape, true);
            context.is_if_else_block.replace(old_val);
            result?
        };

        let mut result = format!("{}{}", cond_str, block_str);

        if let Some(else_block) = self.else_block {
            let shape = Shape::indented(shape.indent, context.config);
            let mut last_in_chain = false;
            let rewrite = match else_block.node {
                // If the else expression is another if-else expression, prevent it
                // from being formatted on a single line.
                // Note how we're passing the original shape, as the
                // cost of "else" should not cascade.
                ast::ExprKind::IfLet(ref pat, ref cond, ref if_block, ref next_else_block) => {
                    ControlFlow::new_if(
                        cond,
                        ptr_vec_to_ref_vec(pat),
                        if_block,
                        next_else_block.as_ref().map(|e| &**e),
                        false,
                        true,
                        mk_sp(else_block.span.lo(), self.span.hi()),
                    ).rewrite(context, shape)
                }
                ast::ExprKind::If(ref cond, ref if_block, ref next_else_block) => {
                    ControlFlow::new_if(
                        cond,
                        vec![],
                        if_block,
                        next_else_block.as_ref().map(|e| &**e),
                        false,
                        true,
                        mk_sp(else_block.span.lo(), self.span.hi()),
                    ).rewrite(context, shape)
                }
                _ => {
                    last_in_chain = true;
                    // When rewriting a block, the width is only used for single line
                    // blocks, passing 1 lets us avoid that.
                    let else_shape = Shape {
                        width: min(1, shape.width),
                        ..shape
                    };
                    format_expr(else_block, ExprType::Statement, context, else_shape)
                }
            };

            let between_kwd_else_block = mk_sp(
                self.block.span.hi(),
                context
                    .snippet_provider
                    .span_before(mk_sp(self.block.span.hi(), else_block.span.lo()), "else"),
            );
            let between_kwd_else_block_comment =
                extract_comment(between_kwd_else_block, context, shape);

            let after_else = mk_sp(
                context
                    .snippet_provider
                    .span_after(mk_sp(self.block.span.hi(), else_block.span.lo()), "else"),
                else_block.span.lo(),
            );
            let after_else_comment = extract_comment(after_else, context, shape);

            let between_sep = match context.config.control_brace_style() {
                ControlBraceStyle::AlwaysNextLine | ControlBraceStyle::ClosingNextLine => {
                    &*alt_block_sep
                }
                ControlBraceStyle::AlwaysSameLine => " ",
            };
            let after_sep = match context.config.control_brace_style() {
                ControlBraceStyle::AlwaysNextLine if last_in_chain => &*alt_block_sep,
                _ => " ",
            };

            result.push_str(&format!(
                "{}else{}",
                between_kwd_else_block_comment
                    .as_ref()
                    .map_or(between_sep, |s| &**s),
                after_else_comment.as_ref().map_or(after_sep, |s| &**s),
            ));
            result.push_str(&rewrite?);
        }

        Some(result)
    }
}

fn rewrite_label(opt_label: Option<ast::Label>) -> Cow<'static, str> {
    match opt_label {
        Some(label) => Cow::from(format!("{}: ", label.ident)),
        None => Cow::from(""),
    }
}

fn extract_comment(span: Span, context: &RewriteContext, shape: Shape) -> Option<String> {
    match rewrite_missing_comment(span, shape, context) {
        Some(ref comment) if !comment.is_empty() => Some(format!(
            "{indent}{}{indent}",
            comment,
            indent = shape.indent.to_string_with_newline(context.config)
        )),
        _ => None,
    }
}

pub fn block_contains_comment(block: &ast::Block, codemap: &CodeMap) -> bool {
    let snippet = codemap.span_to_snippet(block.span).unwrap();
    contains_comment(&snippet)
}

// Checks that a block contains no statements, an expression and no comments or
// attributes.
// FIXME: incorrectly returns false when comment is contained completely within
// the expression.
pub fn is_simple_block(
    block: &ast::Block,
    attrs: Option<&[ast::Attribute]>,
    codemap: &CodeMap,
) -> bool {
    (block.stmts.len() == 1
        && stmt_is_expr(&block.stmts[0])
        && !block_contains_comment(block, codemap)
        && attrs.map_or(true, |a| a.is_empty()))
}

/// Checks whether a block contains at most one statement or expression, and no
/// comments or attributes.
pub fn is_simple_block_stmt(
    block: &ast::Block,
    attrs: Option<&[ast::Attribute]>,
    codemap: &CodeMap,
) -> bool {
    block.stmts.len() <= 1
        && !block_contains_comment(block, codemap)
        && attrs.map_or(true, |a| a.is_empty())
}

/// Checks whether a block contains no statements, expressions, comments, or
/// inner attributes.
pub fn is_empty_block(
    block: &ast::Block,
    attrs: Option<&[ast::Attribute]>,
    codemap: &CodeMap,
) -> bool {
    block.stmts.is_empty()
        && !block_contains_comment(block, codemap)
        && attrs.map_or(true, |a| inner_attributes(a).is_empty())
}

pub fn stmt_is_expr(stmt: &ast::Stmt) -> bool {
    match stmt.node {
        ast::StmtKind::Expr(..) => true,
        _ => false,
    }
}

pub fn is_unsafe_block(block: &ast::Block) -> bool {
    if let ast::BlockCheckMode::Unsafe(..) = block.rules {
        true
    } else {
        false
    }
}

pub fn rewrite_multiple_patterns(
    context: &RewriteContext,
    pats: &[&ast::Pat],
    shape: Shape,
) -> Option<String> {
    let pat_strs = pats
        .iter()
        .map(|p| p.rewrite(context, shape))
        .collect::<Option<Vec<_>>>()?;

    let use_mixed_layout = pats
        .iter()
        .zip(pat_strs.iter())
        .all(|(pat, pat_str)| is_short_pattern(pat, pat_str));
    let items: Vec<_> = pat_strs.into_iter().map(ListItem::from_str).collect();
    let tactic = if use_mixed_layout {
        DefinitiveListTactic::Mixed
    } else {
        definitive_tactic(
            &items,
            ListTactic::HorizontalVertical,
            Separator::VerticalBar,
            shape.width,
        )
    };
    let fmt = ListFormatting {
        tactic,
        separator: " |",
        trailing_separator: SeparatorTactic::Never,
        separator_place: context.config.binop_separator(),
        shape,
        ends_with_newline: false,
        preserve_newline: false,
        nested: false,
        config: context.config,
    };
    write_list(&items, &fmt)
}

pub fn rewrite_literal(context: &RewriteContext, l: &ast::Lit, shape: Shape) -> Option<String> {
    match l.node {
        ast::LitKind::Str(_, ast::StrStyle::Cooked) => rewrite_string_lit(context, l.span, shape),
        _ => wrap_str(
            context.snippet(l.span).to_owned(),
            context.config.max_width(),
            shape,
        ),
    }
}

fn rewrite_string_lit(context: &RewriteContext, span: Span, shape: Shape) -> Option<String> {
    let string_lit = context.snippet(span);

    if !context.config.format_strings() {
        if string_lit
            .lines()
            .rev()
            .skip(1)
            .all(|line| line.ends_with('\\'))
        {
            let new_indent = shape.visual_indent(1).indent;
            let indented_string_lit = String::from(
                string_lit
                    .lines()
                    .map(|line| {
                        format!(
                            "{}{}",
                            new_indent.to_string(context.config),
                            line.trim_left()
                        )
                    }).collect::<Vec<_>>()
                    .join("\n")
                    .trim_left(),
            );
            return wrap_str(indented_string_lit, context.config.max_width(), shape);
        } else {
            return wrap_str(string_lit.to_owned(), context.config.max_width(), shape);
        }
    }

    // Remove the quote characters.
    let str_lit = &string_lit[1..string_lit.len() - 1];

    rewrite_string(
        str_lit,
        &StringFormat::new(shape.visual_indent(0), context.config),
    )
}

/// In case special-case style is required, returns an offset from which we start horizontal layout.
pub fn maybe_get_args_offset<T: ToExpr>(callee_str: &str, args: &[&T]) -> Option<(bool, usize)> {
    if let Some(&(_, num_args_before)) = SPECIAL_MACRO_WHITELIST
        .iter()
        .find(|&&(s, _)| s == callee_str)
    {
        let all_simple = args.len() > num_args_before && is_every_expr_simple(args);

        Some((all_simple, num_args_before))
    } else {
        None
    }
}

/// A list of `format!`-like macros, that take a long format string and a list of arguments to
/// format.
///
/// Organized as a list of `(&str, usize)` tuples, giving the name of the macro and the number of
/// arguments before the format string (none for `format!("format", ...)`, one for `assert!(result,
/// "format", ...)`, two for `assert_eq!(left, right, "format", ...)`).
const SPECIAL_MACRO_WHITELIST: &[(&str, usize)] = &[
    // format! like macros
    // From the Rust Standard Library.
    ("eprint!", 0),
    ("eprintln!", 0),
    ("format!", 0),
    ("format_args!", 0),
    ("print!", 0),
    ("println!", 0),
    ("panic!", 0),
    ("unreachable!", 0),
    // From the `log` crate.
    ("debug!", 0),
    ("error!", 0),
    ("info!", 0),
    ("warn!", 0),
    // write! like macros
    ("assert!", 1),
    ("debug_assert!", 1),
    ("write!", 1),
    ("writeln!", 1),
    // assert_eq! like macros
    ("assert_eq!", 2),
    ("assert_ne!", 2),
    ("debug_assert_eq!", 2),
    ("debug_assert_ne!", 2),
];

fn choose_separator_tactic(context: &RewriteContext, span: Span) -> Option<SeparatorTactic> {
    if context.inside_macro() {
        if span_ends_with_comma(context, span) {
            Some(SeparatorTactic::Always)
        } else {
            Some(SeparatorTactic::Never)
        }
    } else {
        None
    }
}

pub fn rewrite_call(
    context: &RewriteContext,
    callee: &str,
    args: &[ptr::P<ast::Expr>],
    span: Span,
    shape: Shape,
) -> Option<String> {
    overflow::rewrite_with_parens(
        context,
        callee,
        &ptr_vec_to_ref_vec(args),
        shape,
        span,
        context.config.width_heuristics().fn_call_width,
        choose_separator_tactic(context, span),
    )
}

fn is_simple_expr(expr: &ast::Expr) -> bool {
    match expr.node {
        ast::ExprKind::Lit(..) => true,
        ast::ExprKind::Path(ref qself, ref path) => qself.is_none() && path.segments.len() <= 1,
        ast::ExprKind::AddrOf(_, ref expr)
        | ast::ExprKind::Box(ref expr)
        | ast::ExprKind::Cast(ref expr, _)
        | ast::ExprKind::Field(ref expr, _)
        | ast::ExprKind::Try(ref expr)
        | ast::ExprKind::Unary(_, ref expr) => is_simple_expr(expr),
        ast::ExprKind::Index(ref lhs, ref rhs) => is_simple_expr(lhs) && is_simple_expr(rhs),
        ast::ExprKind::Repeat(ref lhs, ref rhs) => {
            is_simple_expr(lhs) && is_simple_expr(&*rhs.value)
        }
        _ => false,
    }
}

pub fn is_every_expr_simple<T: ToExpr>(lists: &[&T]) -> bool {
    lists
        .iter()
        .all(|arg| arg.to_expr().map_or(false, is_simple_expr))
}

pub fn can_be_overflowed_expr(context: &RewriteContext, expr: &ast::Expr, args_len: usize) -> bool {
    match expr.node {
        ast::ExprKind::Match(..) => {
            (context.use_block_indent() && args_len == 1)
                || (context.config.indent_style() == IndentStyle::Visual && args_len > 1)
        }
        ast::ExprKind::If(..)
        | ast::ExprKind::IfLet(..)
        | ast::ExprKind::ForLoop(..)
        | ast::ExprKind::Loop(..)
        | ast::ExprKind::While(..)
        | ast::ExprKind::WhileLet(..) => {
            context.config.combine_control_expr() && context.use_block_indent() && args_len == 1
        }
        ast::ExprKind::Block(..) | ast::ExprKind::Closure(..) => {
            context.use_block_indent()
                || context.config.indent_style() == IndentStyle::Visual && args_len > 1
        }
        ast::ExprKind::Array(..)
        | ast::ExprKind::Call(..)
        | ast::ExprKind::Mac(..)
        | ast::ExprKind::MethodCall(..)
        | ast::ExprKind::Struct(..)
        | ast::ExprKind::Tup(..) => context.use_block_indent() && args_len == 1,
        ast::ExprKind::AddrOf(_, ref expr)
        | ast::ExprKind::Box(ref expr)
        | ast::ExprKind::Try(ref expr)
        | ast::ExprKind::Unary(_, ref expr)
        | ast::ExprKind::Cast(ref expr, _) => can_be_overflowed_expr(context, expr, args_len),
        _ => false,
    }
}

pub fn is_nested_call(expr: &ast::Expr) -> bool {
    match expr.node {
        ast::ExprKind::Call(..) | ast::ExprKind::Mac(..) => true,
        ast::ExprKind::AddrOf(_, ref expr)
        | ast::ExprKind::Box(ref expr)
        | ast::ExprKind::Try(ref expr)
        | ast::ExprKind::Unary(_, ref expr)
        | ast::ExprKind::Cast(ref expr, _) => is_nested_call(expr),
        _ => false,
    }
}

/// Return true if a function call or a method call represented by the given span ends with a
/// trailing comma. This function is used when rewriting macro, as adding or removing a trailing
/// comma from macro can potentially break the code.
pub fn span_ends_with_comma(context: &RewriteContext, span: Span) -> bool {
    let mut result: bool = Default::default();
    let mut prev_char: char = Default::default();
    let closing_delimiters = &[')', '}', ']'];

    for (kind, c) in CharClasses::new(context.snippet(span).chars()) {
        match c {
            _ if kind.is_comment() || c.is_whitespace() => continue,
            c if closing_delimiters.contains(&c) => {
                result &= !closing_delimiters.contains(&prev_char);
            }
            ',' => result = true,
            _ => result = false,
        }
        prev_char = c;
    }

    result
}

fn rewrite_paren(
    context: &RewriteContext,
    mut subexpr: &ast::Expr,
    shape: Shape,
    mut span: Span,
) -> Option<String> {
    debug!("rewrite_paren, shape: {:?}", shape);

    // Extract comments within parens.
    let mut pre_comment;
    let mut post_comment;
    let remove_nested_parens = context.config.remove_nested_parens();
    loop {
        // 1 = "(" or ")"
        let pre_span = mk_sp(span.lo() + BytePos(1), subexpr.span.lo());
        let post_span = mk_sp(subexpr.span.hi(), span.hi() - BytePos(1));
        pre_comment = rewrite_missing_comment(pre_span, shape, context)?;
        post_comment = rewrite_missing_comment(post_span, shape, context)?;

        // Remove nested parens if there are no comments.
        if let ast::ExprKind::Paren(ref subsubexpr) = subexpr.node {
            if remove_nested_parens && pre_comment.is_empty() && post_comment.is_empty() {
                span = subexpr.span;
                subexpr = subsubexpr;
                continue;
            }
        }

        break;
    }

    // 1 `(`
    let sub_shape = shape.offset_left(1).and_then(|s| s.sub_width(1))?;

    let subexpr_str = subexpr.rewrite(context, sub_shape)?;
    debug!("rewrite_paren, subexpr_str: `{:?}`", subexpr_str);

    // 2 = `()`
    if subexpr_str.contains('\n') || first_line_width(&subexpr_str) + 2 <= shape.width {
        Some(format!("({}{}{})", pre_comment, &subexpr_str, post_comment))
    } else {
        None
    }
}

fn rewrite_index(
    expr: &ast::Expr,
    index: &ast::Expr,
    context: &RewriteContext,
    shape: Shape,
) -> Option<String> {
    let expr_str = expr.rewrite(context, shape)?;

    let offset = last_line_width(&expr_str) + 1;
    let rhs_overhead = shape.rhs_overhead(context.config);
    let index_shape = if expr_str.contains('\n') {
        Shape::legacy(context.config.max_width(), shape.indent)
            .offset_left(offset)
            .and_then(|shape| shape.sub_width(1 + rhs_overhead))
    } else {
        shape.visual_indent(offset).sub_width(offset + 1)
    };
    let orig_index_rw = index_shape.and_then(|s| index.rewrite(context, s));

    // Return if index fits in a single line.
    match orig_index_rw {
        Some(ref index_str) if !index_str.contains('\n') => {
            return Some(format!("{}[{}]", expr_str, index_str));
        }
        _ => (),
    }

    // Try putting index on the next line and see if it fits in a single line.
    let indent = shape.indent.block_indent(context.config);
    let index_shape = Shape::indented(indent, context.config).offset_left(1)?;
    let index_shape = index_shape.sub_width(1 + rhs_overhead)?;
    let new_index_rw = index.rewrite(context, index_shape);
    match (orig_index_rw, new_index_rw) {
        (_, Some(ref new_index_str)) if !new_index_str.contains('\n') => Some(format!(
            "{}{}[{}]",
            expr_str,
            indent.to_string_with_newline(context.config),
            new_index_str,
        )),
        (None, Some(ref new_index_str)) => Some(format!(
            "{}{}[{}]",
            expr_str,
            indent.to_string_with_newline(context.config),
            new_index_str,
        )),
        (Some(ref index_str), _) => Some(format!("{}[{}]", expr_str, index_str)),
        _ => None,
    }
}

fn struct_lit_can_be_aligned(fields: &[ast::Field], base: &Option<&ast::Expr>) -> bool {
    if base.is_some() {
        return false;
    }

    fields.iter().all(|field| !field.is_shorthand)
}

fn rewrite_struct_lit<'a>(
    context: &RewriteContext,
    path: &ast::Path,
    fields: &'a [ast::Field],
    base: Option<&'a ast::Expr>,
    span: Span,
    shape: Shape,
) -> Option<String> {
    debug!("rewrite_struct_lit: shape {:?}", shape);

    enum StructLitField<'a> {
        Regular(&'a ast::Field),
        Base(&'a ast::Expr),
    }

    // 2 = " {".len()
    let path_shape = shape.sub_width(2)?;
    let path_str = rewrite_path(context, PathContext::Expr, None, path, path_shape)?;

    if fields.is_empty() && base.is_none() {
        return Some(format!("{} {{}}", path_str));
    }

    // Foo { a: Foo } - indent is +3, width is -5.
    let (h_shape, v_shape) = struct_lit_shape(shape, context, path_str.len() + 3, 2)?;

    let one_line_width = h_shape.map_or(0, |shape| shape.width);
    let body_lo = context.snippet_provider.span_after(span, "{");
    let fields_str = if struct_lit_can_be_aligned(fields, &base)
        && context.config.struct_field_align_threshold() > 0
    {
        rewrite_with_alignment(
            fields,
            context,
            shape,
            mk_sp(body_lo, span.hi()),
            one_line_width,
        )?
    } else {
        let field_iter = fields
            .into_iter()
            .map(StructLitField::Regular)
            .chain(base.into_iter().map(StructLitField::Base));

        let span_lo = |item: &StructLitField| match *item {
            StructLitField::Regular(field) => field.span().lo(),
            StructLitField::Base(expr) => {
                let last_field_hi = fields.last().map_or(span.lo(), |field| field.span.hi());
                let snippet = context.snippet(mk_sp(last_field_hi, expr.span.lo()));
                let pos = snippet.find_uncommented("..").unwrap();
                last_field_hi + BytePos(pos as u32)
            }
        };
        let span_hi = |item: &StructLitField| match *item {
            StructLitField::Regular(field) => field.span().hi(),
            StructLitField::Base(expr) => expr.span.hi(),
        };
        let rewrite = |item: &StructLitField| match *item {
            StructLitField::Regular(field) => {
                // The 1 taken from the v_budget is for the comma.
                rewrite_field(context, field, v_shape.sub_width(1)?, 0)
            }
            StructLitField::Base(expr) => {
                // 2 = ..
                expr.rewrite(context, v_shape.offset_left(2)?)
                    .map(|s| format!("..{}", s))
            }
        };

        let items = itemize_list(
            context.snippet_provider,
            field_iter,
            "}",
            ",",
            span_lo,
            span_hi,
            rewrite,
            body_lo,
            span.hi(),
            false,
        );
        let item_vec = items.collect::<Vec<_>>();

        let tactic = struct_lit_tactic(h_shape, context, &item_vec);
        let nested_shape = shape_for_tactic(tactic, h_shape, v_shape);

        let ends_with_comma = span_ends_with_comma(context, span);
        let force_no_trailing_comma = context.inside_macro() && !ends_with_comma;

        let fmt = struct_lit_formatting(
            nested_shape,
            tactic,
            context,
            force_no_trailing_comma || base.is_some(),
        );

        write_list(&item_vec, &fmt)?
    };

    let fields_str = wrap_struct_field(context, &fields_str, shape, v_shape, one_line_width);
    Some(format!("{} {{{}}}", path_str, fields_str))

    // FIXME if context.config.indent_style() == Visual, but we run out
    // of space, we should fall back to BlockIndent.
}

pub fn wrap_struct_field(
    context: &RewriteContext,
    fields_str: &str,
    shape: Shape,
    nested_shape: Shape,
    one_line_width: usize,
) -> String {
    if context.config.indent_style() == IndentStyle::Block
        && (fields_str.contains('\n')
            || !context.config.struct_lit_single_line()
            || fields_str.len() > one_line_width)
    {
        format!(
            "{}{}{}",
            nested_shape.indent.to_string_with_newline(context.config),
            fields_str,
            shape.indent.to_string_with_newline(context.config)
        )
    } else {
        // One liner or visual indent.
        format!(" {} ", fields_str)
    }
}

pub fn struct_lit_field_separator(config: &Config) -> &str {
    colon_spaces(config.space_before_colon(), config.space_after_colon())
}

pub fn rewrite_field(
    context: &RewriteContext,
    field: &ast::Field,
    shape: Shape,
    prefix_max_width: usize,
) -> Option<String> {
    if contains_skip(&field.attrs) {
        return Some(context.snippet(field.span()).to_owned());
    }
    let mut attrs_str = field.attrs.rewrite(context, shape)?;
    if !attrs_str.is_empty() {
        attrs_str.push_str(&shape.indent.to_string_with_newline(context.config));
    };
    let name = &field.ident.name.to_string();
    if field.is_shorthand {
        Some(attrs_str + &name)
    } else {
        let mut separator = String::from(struct_lit_field_separator(context.config));
        for _ in 0..prefix_max_width.saturating_sub(name.len()) {
            separator.push(' ');
        }
        let overhead = name.len() + separator.len();
        let expr_shape = shape.offset_left(overhead)?;
        let expr = field.expr.rewrite(context, expr_shape);

        match expr {
            Some(ref e) if e.as_str() == name && context.config.use_field_init_shorthand() => {
                Some(attrs_str + &name)
            }
            Some(e) => Some(format!("{}{}{}{}", attrs_str, name, separator, e)),
            None => {
                let expr_offset = shape.indent.block_indent(context.config);
                let expr = field
                    .expr
                    .rewrite(context, Shape::indented(expr_offset, context.config));
                expr.map(|s| {
                    format!(
                        "{}{}:\n{}{}",
                        attrs_str,
                        name,
                        expr_offset.to_string(context.config),
                        s
                    )
                })
            }
        }
    }
}

fn rewrite_tuple_in_visual_indent_style<'a, T>(
    context: &RewriteContext,
    items: &[&T],
    span: Span,
    shape: Shape,
) -> Option<String>
where
    T: Rewrite + Spanned + ToExpr + 'a,
{
    let mut items = items.iter();
    // In case of length 1, need a trailing comma
    debug!("rewrite_tuple_in_visual_indent_style {:?}", shape);
    if items.len() == 1 {
        // 3 = "(" + ",)"
        let nested_shape = shape.sub_width(3)?.visual_indent(1);
        return items
            .next()
            .unwrap()
            .rewrite(context, nested_shape)
            .map(|s| format!("({},)", s));
    }

    let list_lo = context.snippet_provider.span_after(span, "(");
    let nested_shape = shape.sub_width(2)?.visual_indent(1);
    let items = itemize_list(
        context.snippet_provider,
        items,
        ")",
        ",",
        |item| item.span().lo(),
        |item| item.span().hi(),
        |item| item.rewrite(context, nested_shape),
        list_lo,
        span.hi() - BytePos(1),
        false,
    );
    let item_vec: Vec<_> = items.collect();
    let tactic = definitive_tactic(
        &item_vec,
        ListTactic::HorizontalVertical,
        Separator::Comma,
        nested_shape.width,
    );
    let fmt = ListFormatting {
        tactic,
        separator: ",",
        trailing_separator: SeparatorTactic::Never,
        separator_place: SeparatorPlace::Back,
        shape,
        ends_with_newline: false,
        preserve_newline: false,
        nested: false,
        config: context.config,
    };
    let list_str = write_list(&item_vec, &fmt)?;

    Some(format!("({})", list_str))
}

pub fn rewrite_tuple<'a, T>(
    context: &RewriteContext,
    items: &[&T],
    span: Span,
    shape: Shape,
) -> Option<String>
where
    T: Rewrite + Spanned + ToExpr + 'a,
{
    debug!("rewrite_tuple {:?}", shape);
    if context.use_block_indent() {
        // We use the same rule as function calls for rewriting tuples.
        let force_tactic = if context.inside_macro() {
            if span_ends_with_comma(context, span) {
                Some(SeparatorTactic::Always)
            } else {
                Some(SeparatorTactic::Never)
            }
        } else if items.len() == 1 {
            Some(SeparatorTactic::Always)
        } else {
            None
        };
        overflow::rewrite_with_parens(
            context,
            "",
            items,
            shape,
            span,
            context.config.width_heuristics().fn_call_width,
            force_tactic,
        )
    } else {
        rewrite_tuple_in_visual_indent_style(context, items, span, shape)
    }
}

pub fn rewrite_unary_prefix<R: Rewrite>(
    context: &RewriteContext,
    prefix: &str,
    rewrite: &R,
    shape: Shape,
) -> Option<String> {
    rewrite
        .rewrite(context, shape.offset_left(prefix.len())?)
        .map(|r| format!("{}{}", prefix, r))
}

// FIXME: this is probably not correct for multi-line Rewrites. we should
// subtract suffix.len() from the last line budget, not the first!
pub fn rewrite_unary_suffix<R: Rewrite>(
    context: &RewriteContext,
    suffix: &str,
    rewrite: &R,
    shape: Shape,
) -> Option<String> {
    rewrite
        .rewrite(context, shape.sub_width(suffix.len())?)
        .map(|mut r| {
            r.push_str(suffix);
            r
        })
}

fn rewrite_unary_op(
    context: &RewriteContext,
    op: &ast::UnOp,
    expr: &ast::Expr,
    shape: Shape,
) -> Option<String> {
    // For some reason, an UnOp is not spanned like BinOp!
    let operator_str = match *op {
        ast::UnOp::Deref => "*",
        ast::UnOp::Not => "!",
        ast::UnOp::Neg => "-",
    };
    rewrite_unary_prefix(context, operator_str, expr, shape)
}

fn rewrite_assignment(
    context: &RewriteContext,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    op: Option<&ast::BinOp>,
    shape: Shape,
) -> Option<String> {
    let operator_str = match op {
        Some(op) => context.snippet(op.span),
        None => "=",
    };

    // 1 = space between lhs and operator.
    let lhs_shape = shape.sub_width(operator_str.len() + 1)?;
    let lhs_str = format!("{} {}", lhs.rewrite(context, lhs_shape)?, operator_str);

    rewrite_assign_rhs(context, lhs_str, rhs, shape)
}

/// Controls where to put the rhs.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RhsTactics {
    /// Use heuristics.
    Default,
    /// Put the rhs on the next line if it uses multiple line, without extra indentation.
    ForceNextLineWithoutIndent,
}

// The left hand side must contain everything up to, and including, the
// assignment operator.
pub fn rewrite_assign_rhs<S: Into<String>, R: Rewrite>(
    context: &RewriteContext,
    lhs: S,
    ex: &R,
    shape: Shape,
) -> Option<String> {
    rewrite_assign_rhs_with(context, lhs, ex, shape, RhsTactics::Default)
}

pub fn rewrite_assign_rhs_with<S: Into<String>, R: Rewrite>(
    context: &RewriteContext,
    lhs: S,
    ex: &R,
    shape: Shape,
    rhs_tactics: RhsTactics,
) -> Option<String> {
    let lhs = lhs.into();
    let last_line_width = last_line_width(&lhs).saturating_sub(if lhs.contains('\n') {
        shape.indent.width()
    } else {
        0
    });
    // 1 = space between operator and rhs.
    let orig_shape = shape.offset_left(last_line_width + 1).unwrap_or(Shape {
        width: 0,
        offset: shape.offset + last_line_width + 1,
        ..shape
    });
    let rhs = choose_rhs(
        context,
        ex,
        orig_shape,
        ex.rewrite(context, orig_shape),
        rhs_tactics,
    )?;
    Some(lhs + &rhs)
}

fn choose_rhs<R: Rewrite>(
    context: &RewriteContext,
    expr: &R,
    shape: Shape,
    orig_rhs: Option<String>,
    rhs_tactics: RhsTactics,
) -> Option<String> {
    match orig_rhs {
        Some(ref new_str) if !new_str.contains('\n') && new_str.len() <= shape.width => {
            Some(format!(" {}", new_str))
        }
        _ => {
            // Expression did not fit on the same line as the identifier.
            // Try splitting the line and see if that works better.
            let new_shape = shape_from_rhs_tactic(context, shape, rhs_tactics)?;
            let new_rhs = expr.rewrite(context, new_shape);
            let new_indent_str = &shape
                .indent
                .block_indent(context.config)
                .to_string_with_newline(context.config);

            match (orig_rhs, new_rhs) {
                (Some(ref orig_rhs), Some(ref new_rhs))
                    if wrap_str(new_rhs.clone(), context.config.max_width(), new_shape)
                        .is_none() =>
                {
                    Some(format!(" {}", orig_rhs))
                }
                (Some(ref orig_rhs), Some(ref new_rhs))
                    if prefer_next_line(orig_rhs, new_rhs, rhs_tactics) =>
                {
                    Some(format!("{}{}", new_indent_str, new_rhs))
                }
                (None, Some(ref new_rhs)) => Some(format!("{}{}", new_indent_str, new_rhs)),
                (None, None) => None,
                (Some(orig_rhs), _) => Some(format!(" {}", orig_rhs)),
            }
        }
    }
}

fn shape_from_rhs_tactic(
    context: &RewriteContext,
    shape: Shape,
    rhs_tactic: RhsTactics,
) -> Option<Shape> {
    match rhs_tactic {
        RhsTactics::ForceNextLineWithoutIndent => Some(shape.with_max_width(context.config)),
        RhsTactics::Default => {
            Shape::indented(shape.indent.block_indent(context.config), context.config)
                .sub_width(shape.rhs_overhead(context.config))
        }
    }
}

pub fn prefer_next_line(orig_rhs: &str, next_line_rhs: &str, rhs_tactics: RhsTactics) -> bool {
    rhs_tactics == RhsTactics::ForceNextLineWithoutIndent
        || !next_line_rhs.contains('\n')
        || count_newlines(orig_rhs) > count_newlines(next_line_rhs) + 1
        || first_line_ends_with(orig_rhs, '(') && !first_line_ends_with(next_line_rhs, '(')
        || first_line_ends_with(orig_rhs, '{') && !first_line_ends_with(next_line_rhs, '{')
        || first_line_ends_with(orig_rhs, '[') && !first_line_ends_with(next_line_rhs, '[')
}

fn rewrite_expr_addrof(
    context: &RewriteContext,
    mutability: ast::Mutability,
    expr: &ast::Expr,
    shape: Shape,
) -> Option<String> {
    let operator_str = match mutability {
        ast::Mutability::Immutable => "&",
        ast::Mutability::Mutable => "&mut ",
    };
    rewrite_unary_prefix(context, operator_str, expr, shape)
}

pub trait ToExpr {
    fn to_expr(&self) -> Option<&ast::Expr>;
    fn can_be_overflowed(&self, context: &RewriteContext, len: usize) -> bool;
}

impl ToExpr for ast::Expr {
    fn to_expr(&self) -> Option<&ast::Expr> {
        Some(self)
    }

    fn can_be_overflowed(&self, context: &RewriteContext, len: usize) -> bool {
        can_be_overflowed_expr(context, self, len)
    }
}

impl ToExpr for ast::Ty {
    fn to_expr(&self) -> Option<&ast::Expr> {
        None
    }

    fn can_be_overflowed(&self, context: &RewriteContext, len: usize) -> bool {
        can_be_overflowed_type(context, self, len)
    }
}

impl<'a> ToExpr for TuplePatField<'a> {
    fn to_expr(&self) -> Option<&ast::Expr> {
        None
    }

    fn can_be_overflowed(&self, context: &RewriteContext, len: usize) -> bool {
        can_be_overflowed_pat(context, self, len)
    }
}

impl<'a> ToExpr for ast::StructField {
    fn to_expr(&self) -> Option<&ast::Expr> {
        None
    }

    fn can_be_overflowed(&self, _: &RewriteContext, _: usize) -> bool {
        false
    }
}

impl<'a> ToExpr for MacroArg {
    fn to_expr(&self) -> Option<&ast::Expr> {
        match *self {
            MacroArg::Expr(ref expr) => Some(expr),
            _ => None,
        }
    }

    fn can_be_overflowed(&self, context: &RewriteContext, len: usize) -> bool {
        match *self {
            MacroArg::Expr(ref expr) => can_be_overflowed_expr(context, expr, len),
            MacroArg::Ty(ref ty) => can_be_overflowed_type(context, ty, len),
            MacroArg::Pat(..) => false,
            MacroArg::Item(..) => len == 1,
        }
    }
}

impl ToExpr for ast::GenericParam {
    fn to_expr(&self) -> Option<&ast::Expr> {
        None
    }

    fn can_be_overflowed(&self, _: &RewriteContext, _: usize) -> bool {
        false
    }
}

pub fn is_method_call(expr: &ast::Expr) -> bool {
    match expr.node {
        ast::ExprKind::MethodCall(..) => true,
        ast::ExprKind::AddrOf(_, ref expr)
        | ast::ExprKind::Box(ref expr)
        | ast::ExprKind::Cast(ref expr, _)
        | ast::ExprKind::Try(ref expr)
        | ast::ExprKind::Unary(_, ref expr) => is_method_call(expr),
        _ => false,
    }
}
