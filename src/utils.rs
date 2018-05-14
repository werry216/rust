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

use rustc_target::spec::abi;
use syntax::ast::{
    self, Attribute, CrateSugar, MetaItem, MetaItemKind, NestedMetaItem, NestedMetaItemKind, Path,
    Visibility, VisibilityKind,
};
use syntax::codemap::{BytePos, Span, NO_EXPANSION};
use syntax::ptr;

use config::Color;
use rewrite::RewriteContext;
use shape::Shape;

const DEPR_SKIP_ANNOTATION: &str = "rustfmt_skip";
const SKIP_ANNOTATION: &str = "rustfmt::skip";

// Computes the length of a string's last line, minus offset.
pub fn extra_offset(text: &str, shape: Shape) -> usize {
    match text.rfind('\n') {
        // 1 for newline character
        Some(idx) => text
            .len()
            .checked_sub(idx + 1 + shape.used_width())
            .unwrap_or(0),
        None => text.len(),
    }
}

// Uses Cow to avoid allocating in the common cases.
pub fn format_visibility(vis: &Visibility) -> Cow<'static, str> {
    match vis.node {
        VisibilityKind::Public => Cow::from("pub "),
        VisibilityKind::Inherited => Cow::from(""),
        VisibilityKind::Crate(CrateSugar::PubCrate) => Cow::from("pub(crate) "),
        VisibilityKind::Crate(CrateSugar::JustCrate) => Cow::from("crate "),
        VisibilityKind::Restricted { ref path, .. } => {
            let Path { ref segments, .. } = **path;
            let mut segments_iter = segments.iter().map(|seg| seg.ident.name.to_string());
            if path.is_global() {
                segments_iter
                    .next()
                    .expect("Non-global path in pub(restricted)?");
            }
            let is_keyword = |s: &str| s == "self" || s == "super";
            let path = segments_iter.collect::<Vec<_>>().join("::");
            let in_str = if is_keyword(&path) { "" } else { "in " };

            Cow::from(format!("pub({}{}) ", in_str, path))
        }
    }
}

#[inline]
pub fn format_constness(constness: ast::Constness) -> &'static str {
    match constness {
        ast::Constness::Const => "const ",
        ast::Constness::NotConst => "",
    }
}

#[inline]
pub fn format_defaultness(defaultness: ast::Defaultness) -> &'static str {
    match defaultness {
        ast::Defaultness::Default => "default ",
        ast::Defaultness::Final => "",
    }
}

#[inline]
pub fn format_unsafety(unsafety: ast::Unsafety) -> &'static str {
    match unsafety {
        ast::Unsafety::Unsafe => "unsafe ",
        ast::Unsafety::Normal => "",
    }
}

#[inline]
pub fn format_auto(is_auto: ast::IsAuto) -> &'static str {
    match is_auto {
        ast::IsAuto::Yes => "auto ",
        ast::IsAuto::No => "",
    }
}

#[inline]
pub fn format_mutability(mutability: ast::Mutability) -> &'static str {
    match mutability {
        ast::Mutability::Mutable => "mut ",
        ast::Mutability::Immutable => "",
    }
}

#[inline]
pub fn format_abi(abi: abi::Abi, explicit_abi: bool, is_mod: bool) -> Cow<'static, str> {
    if abi == abi::Abi::Rust && !is_mod {
        Cow::from("")
    } else if abi == abi::Abi::C && !explicit_abi {
        Cow::from("extern ")
    } else {
        Cow::from(format!("extern {} ", abi))
    }
}

#[inline]
// Transform `Vec<syntax::ptr::P<T>>` into `Vec<&T>`
pub fn ptr_vec_to_ref_vec<T>(vec: &[ptr::P<T>]) -> Vec<&T> {
    vec.iter().map(|x| &**x).collect::<Vec<_>>()
}

#[inline]
pub fn filter_attributes(attrs: &[ast::Attribute], style: ast::AttrStyle) -> Vec<ast::Attribute> {
    attrs
        .iter()
        .filter(|a| a.style == style)
        .cloned()
        .collect::<Vec<_>>()
}

#[inline]
pub fn inner_attributes(attrs: &[ast::Attribute]) -> Vec<ast::Attribute> {
    filter_attributes(attrs, ast::AttrStyle::Inner)
}

#[inline]
pub fn outer_attributes(attrs: &[ast::Attribute]) -> Vec<ast::Attribute> {
    filter_attributes(attrs, ast::AttrStyle::Outer)
}

#[inline]
pub fn is_single_line(s: &str) -> bool {
    s.chars().find(|&c| c == '\n').is_none()
}

#[inline]
pub fn first_line_contains_single_line_comment(s: &str) -> bool {
    s.lines().next().map_or(false, |l| l.contains("//"))
}

#[inline]
pub fn last_line_contains_single_line_comment(s: &str) -> bool {
    s.lines().last().map_or(false, |l| l.contains("//"))
}

#[inline]
pub fn is_attributes_extendable(attrs_str: &str) -> bool {
    !attrs_str.contains('\n') && !last_line_contains_single_line_comment(attrs_str)
}

// The width of the first line in s.
#[inline]
pub fn first_line_width(s: &str) -> usize {
    match s.find('\n') {
        Some(n) => n,
        None => s.len(),
    }
}

// The width of the last line in s.
#[inline]
pub fn last_line_width(s: &str) -> usize {
    match s.rfind('\n') {
        Some(n) => s.len() - n - 1,
        None => s.len(),
    }
}

// The total used width of the last line.
#[inline]
pub fn last_line_used_width(s: &str, offset: usize) -> usize {
    if s.contains('\n') {
        last_line_width(s)
    } else {
        offset + s.len()
    }
}

#[inline]
pub fn trimmed_last_line_width(s: &str) -> usize {
    match s.rfind('\n') {
        Some(n) => s[(n + 1)..].trim().len(),
        None => s.trim().len(),
    }
}

#[inline]
pub fn last_line_extendable(s: &str) -> bool {
    if s.ends_with("\"#") {
        return true;
    }
    for c in s.chars().rev() {
        match c {
            '(' | ')' | ']' | '}' | '?' | '>' => continue,
            '\n' => break,
            _ if c.is_whitespace() => continue,
            _ => return false,
        }
    }
    true
}

#[inline]
fn is_skip(meta_item: &MetaItem) -> bool {
    match meta_item.node {
        MetaItemKind::Word => {
            let path_str = meta_item.ident.to_string();
            path_str == SKIP_ANNOTATION || path_str == DEPR_SKIP_ANNOTATION
        }
        MetaItemKind::List(ref l) => {
            meta_item.name() == "cfg_attr" && l.len() == 2 && is_skip_nested(&l[1])
        }
        _ => false,
    }
}

#[inline]
fn is_skip_nested(meta_item: &NestedMetaItem) -> bool {
    match meta_item.node {
        NestedMetaItemKind::MetaItem(ref mi) => is_skip(mi),
        NestedMetaItemKind::Literal(_) => false,
    }
}

#[inline]
pub fn contains_skip(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|a| a.meta().map_or(false, |a| is_skip(&a)))
}

#[inline]
pub fn semicolon_for_expr(context: &RewriteContext, expr: &ast::Expr) -> bool {
    match expr.node {
        ast::ExprKind::Ret(..) | ast::ExprKind::Continue(..) | ast::ExprKind::Break(..) => {
            context.config.trailing_semicolon()
        }
        _ => false,
    }
}

#[inline]
pub fn semicolon_for_stmt(context: &RewriteContext, stmt: &ast::Stmt) -> bool {
    match stmt.node {
        ast::StmtKind::Semi(ref expr) => match expr.node {
            ast::ExprKind::While(..)
            | ast::ExprKind::WhileLet(..)
            | ast::ExprKind::Loop(..)
            | ast::ExprKind::ForLoop(..) => false,
            ast::ExprKind::Break(..) | ast::ExprKind::Continue(..) | ast::ExprKind::Ret(..) => {
                context.config.trailing_semicolon()
            }
            _ => true,
        },
        ast::StmtKind::Expr(..) => false,
        _ => true,
    }
}

#[inline]
pub fn stmt_expr(stmt: &ast::Stmt) -> Option<&ast::Expr> {
    match stmt.node {
        ast::StmtKind::Expr(ref expr) => Some(expr),
        _ => None,
    }
}

#[inline]
pub fn count_newlines(input: &str) -> usize {
    // Using `as_bytes` to omit UTF-8 decoding
    input.as_bytes().iter().filter(|&&c| c == b'\n').count()
}

// For format_missing and last_pos, need to use the source callsite (if applicable).
// Required as generated code spans aren't guaranteed to follow on from the last span.
macro_rules! source {
    ($this:ident, $sp:expr) => {
        $sp.source_callsite()
    };
}

pub fn mk_sp(lo: BytePos, hi: BytePos) -> Span {
    Span::new(lo, hi, NO_EXPANSION)
}

// Return true if the given span does not intersect with file lines.
macro_rules! out_of_file_lines_range {
    ($self:ident, $span:expr) => {
        !$self.config.file_lines().is_all()
            && !$self
                .config
                .file_lines()
                .intersects(&$self.codemap.lookup_line_range($span))
    };
}

macro_rules! skip_out_of_file_lines_range {
    ($self:ident, $span:expr) => {
        if out_of_file_lines_range!($self, $span) {
            return None;
        }
    };
}

macro_rules! skip_out_of_file_lines_range_visitor {
    ($self:ident, $span:expr) => {
        if out_of_file_lines_range!($self, $span) {
            $self.push_rewrite($span, None);
            return;
        }
    };
}

// Wraps String in an Option. Returns Some when the string adheres to the
// Rewrite constraints defined for the Rewrite trait and else otherwise.
pub fn wrap_str(s: String, max_width: usize, shape: Shape) -> Option<String> {
    if is_valid_str(&s, max_width, shape) {
        Some(s)
    } else {
        None
    }
}

fn is_valid_str(snippet: &str, max_width: usize, shape: Shape) -> bool {
    if !snippet.is_empty() {
        // First line must fits with `shape.width`.
        if first_line_width(snippet) > shape.width {
            return false;
        }
        // If the snippet does not include newline, we are done.
        if first_line_width(snippet) == snippet.len() {
            return true;
        }
        // The other lines must fit within the maximum width.
        if snippet.lines().skip(1).any(|line| line.len() > max_width) {
            return false;
        }
        // A special check for the last line, since the caller may
        // place trailing characters on this line.
        if last_line_width(snippet) > shape.used_width() + shape.width {
            return false;
        }
    }
    true
}

#[inline]
pub fn colon_spaces(before: bool, after: bool) -> &'static str {
    match (before, after) {
        (true, true) => " : ",
        (true, false) => " :",
        (false, true) => ": ",
        (false, false) => ":",
    }
}

#[inline]
pub fn paren_overhead(context: &RewriteContext) -> usize {
    if context.config.spaces_within_parens_and_brackets() {
        4
    } else {
        2
    }
}

pub fn left_most_sub_expr(e: &ast::Expr) -> &ast::Expr {
    match e.node {
        ast::ExprKind::Call(ref e, _)
        | ast::ExprKind::Binary(_, ref e, _)
        | ast::ExprKind::Cast(ref e, _)
        | ast::ExprKind::Type(ref e, _)
        | ast::ExprKind::Assign(ref e, _)
        | ast::ExprKind::AssignOp(_, ref e, _)
        | ast::ExprKind::Field(ref e, _)
        | ast::ExprKind::Index(ref e, _)
        | ast::ExprKind::Range(Some(ref e), _, _)
        | ast::ExprKind::Try(ref e) => left_most_sub_expr(e),
        _ => e,
    }
}

// isatty shamelessly adapted from cargo.
#[cfg(unix)]
pub fn isatty() -> bool {
    extern crate libc;

    unsafe { libc::isatty(libc::STDOUT_FILENO) != 0 }
}
#[cfg(windows)]
pub fn isatty() -> bool {
    extern crate winapi;

    unsafe {
        let handle = winapi::um::processenv::GetStdHandle(winapi::um::winbase::STD_OUTPUT_HANDLE);
        let mut out = 0;
        winapi::um::consoleapi::GetConsoleMode(handle, &mut out) != 0
    }
}

pub fn use_colored_tty(color: Color) -> bool {
    match color {
        Color::Always => true,
        Color::Never => false,
        Color::Auto => isatty(),
    }
}

pub fn starts_with_newline(s: &str) -> bool {
    s.starts_with('\n') || s.starts_with("\r\n")
}
