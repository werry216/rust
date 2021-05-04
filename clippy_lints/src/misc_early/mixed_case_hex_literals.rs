use clippy_utils::diagnostics::span_lint;
use rustc_ast::ast::Lit;
use rustc_lint::EarlyContext;

use super::MIXED_CASE_HEX_LITERALS;

pub(super) fn check(cx: &EarlyContext<'_>, lit: &Lit, maybe_last_sep_idx: usize, lit_snip: String) {
    if maybe_last_sep_idx <= 2 {
        // It's meaningless or causes range error.
        return;
    }
    let mut seen = (false, false);
    for ch in lit_snip.as_bytes()[2..=maybe_last_sep_idx].iter() {
        match ch {
            b'a'..=b'f' => seen.0 = true,
            b'A'..=b'F' => seen.1 = true,
            _ => {},
        }
        if seen.0 && seen.1 {
            span_lint(
                cx,
                MIXED_CASE_HEX_LITERALS,
                lit.span,
                "inconsistent casing in hexadecimal literal",
            );
            break;
        }
    }
}
