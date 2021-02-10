mod crosspointer_transmute;
mod transmute_int_to_char;
mod transmute_ptr_to_ref;
mod useless_transmute;
mod utils;
mod wrong_transmute;

use utils::*;

use crate::utils::{
    in_constant, match_def_path, paths, snippet, span_lint, span_lint_and_sugg, span_lint_and_then, sugg,
};
use if_chain::if_chain;
use rustc_ast as ast;
use rustc_errors::Applicability;
use rustc_hir::{Expr, ExprKind, Mutability, UnOp};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_session::{declare_lint_pass, declare_tool_lint};
use std::borrow::Cow;

declare_clippy_lint! {
    /// **What it does:** Checks for transmutes that can't ever be correct on any
    /// architecture.
    ///
    /// **Why is this bad?** It's basically guaranteed to be undefined behaviour.
    ///
    /// **Known problems:** When accessing C, users might want to store pointer
    /// sized objects in `extradata` arguments to save an allocation.
    ///
    /// **Example:**
    /// ```ignore
    /// let ptr: *const T = core::intrinsics::transmute('x')
    /// ```
    pub WRONG_TRANSMUTE,
    correctness,
    "transmutes that are confusing at best, undefined behaviour at worst and always useless"
}

// FIXME: Move this to `complexity` again, after #5343 is fixed
declare_clippy_lint! {
    /// **What it does:** Checks for transmutes to the original type of the object
    /// and transmutes that could be a cast.
    ///
    /// **Why is this bad?** Readability. The code tricks people into thinking that
    /// something complex is going on.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust,ignore
    /// core::intrinsics::transmute(t); // where the result type is the same as `t`'s
    /// ```
    pub USELESS_TRANSMUTE,
    nursery,
    "transmutes that have the same to and from types or could be a cast/coercion"
}

// FIXME: Merge this lint with USELESS_TRANSMUTE once that is out of the nursery.
declare_clippy_lint! {
    /// **What it does:**Checks for transmutes that could be a pointer cast.
    ///
    /// **Why is this bad?** Readability. The code tricks people into thinking that
    /// something complex is going on.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// # let p: *const [i32] = &[];
    /// unsafe { std::mem::transmute::<*const [i32], *const [u16]>(p) };
    /// ```
    /// Use instead:
    /// ```rust
    /// # let p: *const [i32] = &[];
    /// p as *const [u16];
    /// ```
    pub TRANSMUTES_EXPRESSIBLE_AS_PTR_CASTS,
    complexity,
    "transmutes that could be a pointer cast"
}

declare_clippy_lint! {
    /// **What it does:** Checks for transmutes between a type `T` and `*T`.
    ///
    /// **Why is this bad?** It's easy to mistakenly transmute between a type and a
    /// pointer to that type.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust,ignore
    /// core::intrinsics::transmute(t) // where the result type is the same as
    ///                                // `*t` or `&t`'s
    /// ```
    pub CROSSPOINTER_TRANSMUTE,
    complexity,
    "transmutes that have to or from types that are a pointer to the other"
}

declare_clippy_lint! {
    /// **What it does:** Checks for transmutes from a pointer to a reference.
    ///
    /// **Why is this bad?** This can always be rewritten with `&` and `*`.
    ///
    /// **Known problems:**
    /// - `mem::transmute` in statics and constants is stable from Rust 1.46.0,
    /// while dereferencing raw pointer is not stable yet.
    /// If you need to do this in those places,
    /// you would have to use `transmute` instead.
    ///
    /// **Example:**
    /// ```rust,ignore
    /// unsafe {
    ///     let _: &T = std::mem::transmute(p); // where p: *const T
    /// }
    ///
    /// // can be written:
    /// let _: &T = &*p;
    /// ```
    pub TRANSMUTE_PTR_TO_REF,
    complexity,
    "transmutes from a pointer to a reference type"
}

declare_clippy_lint! {
    /// **What it does:** Checks for transmutes from an integer to a `char`.
    ///
    /// **Why is this bad?** Not every integer is a Unicode scalar value.
    ///
    /// **Known problems:**
    /// - [`from_u32`] which this lint suggests using is slower than `transmute`
    /// as it needs to validate the input.
    /// If you are certain that the input is always a valid Unicode scalar value,
    /// use [`from_u32_unchecked`] which is as fast as `transmute`
    /// but has a semantically meaningful name.
    /// - You might want to handle `None` returned from [`from_u32`] instead of calling `unwrap`.
    ///
    /// [`from_u32`]: https://doc.rust-lang.org/std/char/fn.from_u32.html
    /// [`from_u32_unchecked`]: https://doc.rust-lang.org/std/char/fn.from_u32_unchecked.html
    ///
    /// **Example:**
    /// ```rust
    /// let x = 1_u32;
    /// unsafe {
    ///     let _: char = std::mem::transmute(x); // where x: u32
    /// }
    ///
    /// // should be:
    /// let _ = std::char::from_u32(x).unwrap();
    /// ```
    pub TRANSMUTE_INT_TO_CHAR,
    complexity,
    "transmutes from an integer to a `char`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for transmutes from a `&[u8]` to a `&str`.
    ///
    /// **Why is this bad?** Not every byte slice is a valid UTF-8 string.
    ///
    /// **Known problems:**
    /// - [`from_utf8`] which this lint suggests using is slower than `transmute`
    /// as it needs to validate the input.
    /// If you are certain that the input is always a valid UTF-8,
    /// use [`from_utf8_unchecked`] which is as fast as `transmute`
    /// but has a semantically meaningful name.
    /// - You might want to handle errors returned from [`from_utf8`] instead of calling `unwrap`.
    ///
    /// [`from_utf8`]: https://doc.rust-lang.org/std/str/fn.from_utf8.html
    /// [`from_utf8_unchecked`]: https://doc.rust-lang.org/std/str/fn.from_utf8_unchecked.html
    ///
    /// **Example:**
    /// ```rust
    /// let b: &[u8] = &[1_u8, 2_u8];
    /// unsafe {
    ///     let _: &str = std::mem::transmute(b); // where b: &[u8]
    /// }
    ///
    /// // should be:
    /// let _ = std::str::from_utf8(b).unwrap();
    /// ```
    pub TRANSMUTE_BYTES_TO_STR,
    complexity,
    "transmutes from a `&[u8]` to a `&str`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for transmutes from an integer to a `bool`.
    ///
    /// **Why is this bad?** This might result in an invalid in-memory representation of a `bool`.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// let x = 1_u8;
    /// unsafe {
    ///     let _: bool = std::mem::transmute(x); // where x: u8
    /// }
    ///
    /// // should be:
    /// let _: bool = x != 0;
    /// ```
    pub TRANSMUTE_INT_TO_BOOL,
    complexity,
    "transmutes from an integer to a `bool`"
}

declare_clippy_lint! {
    /// **What it does:** Checks for transmutes from an integer to a float.
    ///
    /// **Why is this bad?** Transmutes are dangerous and error-prone, whereas `from_bits` is intuitive
    /// and safe.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// unsafe {
    ///     let _: f32 = std::mem::transmute(1_u32); // where x: u32
    /// }
    ///
    /// // should be:
    /// let _: f32 = f32::from_bits(1_u32);
    /// ```
    pub TRANSMUTE_INT_TO_FLOAT,
    complexity,
    "transmutes from an integer to a float"
}

declare_clippy_lint! {
    /// **What it does:** Checks for transmutes from a float to an integer.
    ///
    /// **Why is this bad?** Transmutes are dangerous and error-prone, whereas `to_bits` is intuitive
    /// and safe.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// unsafe {
    ///     let _: u32 = std::mem::transmute(1f32);
    /// }
    ///
    /// // should be:
    /// let _: u32 = 1f32.to_bits();
    /// ```
    pub TRANSMUTE_FLOAT_TO_INT,
    complexity,
    "transmutes from a float to an integer"
}

declare_clippy_lint! {
    /// **What it does:** Checks for transmutes from a pointer to a pointer, or
    /// from a reference to a reference.
    ///
    /// **Why is this bad?** Transmutes are dangerous, and these can instead be
    /// written as casts.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// let ptr = &1u32 as *const u32;
    /// unsafe {
    ///     // pointer-to-pointer transmute
    ///     let _: *const f32 = std::mem::transmute(ptr);
    ///     // ref-ref transmute
    ///     let _: &f32 = std::mem::transmute(&1u32);
    /// }
    /// // These can be respectively written:
    /// let _ = ptr as *const f32;
    /// let _ = unsafe{ &*(&1u32 as *const u32 as *const f32) };
    /// ```
    pub TRANSMUTE_PTR_TO_PTR,
    complexity,
    "transmutes from a pointer to a pointer / a reference to a reference"
}

declare_clippy_lint! {
    /// **What it does:** Checks for transmutes between collections whose
    /// types have different ABI, size or alignment.
    ///
    /// **Why is this bad?** This is undefined behavior.
    ///
    /// **Known problems:** Currently, we cannot know whether a type is a
    /// collection, so we just lint the ones that come with `std`.
    ///
    /// **Example:**
    /// ```rust
    /// // different size, therefore likely out-of-bounds memory access
    /// // You absolutely do not want this in your code!
    /// unsafe {
    ///     std::mem::transmute::<_, Vec<u32>>(vec![2_u16])
    /// };
    /// ```
    ///
    /// You must always iterate, map and collect the values:
    ///
    /// ```rust
    /// vec![2_u16].into_iter().map(u32::from).collect::<Vec<_>>();
    /// ```
    pub UNSOUND_COLLECTION_TRANSMUTE,
    correctness,
    "transmute between collections of layout-incompatible types"
}

declare_lint_pass!(Transmute => [
    CROSSPOINTER_TRANSMUTE,
    TRANSMUTE_PTR_TO_REF,
    TRANSMUTE_PTR_TO_PTR,
    USELESS_TRANSMUTE,
    WRONG_TRANSMUTE,
    TRANSMUTE_INT_TO_CHAR,
    TRANSMUTE_BYTES_TO_STR,
    TRANSMUTE_INT_TO_BOOL,
    TRANSMUTE_INT_TO_FLOAT,
    TRANSMUTE_FLOAT_TO_INT,
    UNSOUND_COLLECTION_TRANSMUTE,
    TRANSMUTES_EXPRESSIBLE_AS_PTR_CASTS,
]);

// used to check for UNSOUND_COLLECTION_TRANSMUTE
static COLLECTIONS: &[&[&str]] = &[
    &paths::VEC,
    &paths::VEC_DEQUE,
    &paths::BINARY_HEAP,
    &paths::BTREESET,
    &paths::BTREEMAP,
    &paths::HASHSET,
    &paths::HASHMAP,
];

impl<'tcx> LateLintPass<'tcx> for Transmute {
    #[allow(clippy::similar_names, clippy::too_many_lines)]
    fn check_expr(&mut self, cx: &LateContext<'tcx>, e: &'tcx Expr<'_>) {
        if_chain! {
            if let ExprKind::Call(ref path_expr, ref args) = e.kind;
            if let ExprKind::Path(ref qpath) = path_expr.kind;
            if let Some(def_id) = cx.qpath_res(qpath, path_expr.hir_id).opt_def_id();
            if match_def_path(cx, def_id, &paths::TRANSMUTE);
            then {
                // Avoid suggesting from/to bits and dereferencing raw pointers in const contexts.
                // See https://github.com/rust-lang/rust/issues/73736 for progress on making them `const fn`.
                // And see https://github.com/rust-lang/rust/issues/51911 for dereferencing raw pointers.
                let const_context = in_constant(cx, e.hir_id);

                let from_ty = cx.typeck_results().expr_ty(&args[0]);
                let to_ty = cx.typeck_results().expr_ty(e);

                let triggered = useless_transmute::check(cx, e, from_ty, to_ty, args);
                if triggered {
                    return;
                }
                let triggered = wrong_transmute::check(cx, e, from_ty, to_ty);
                if triggered {
                    return;
                }
                let triggered = crosspointer_transmute::check(cx, e, from_ty, to_ty);
                if triggered {
                    return;
                }
                let triggered = transmute_ptr_to_ref::check(cx, e, from_ty, to_ty, args, qpath);
                if triggered {
                    return;
                }
                let triggered = transmute_int_to_char::check(cx, e, from_ty, to_ty, args);
                if triggered {
                    return;
                }

                match (&from_ty.kind(), &to_ty.kind()) {
                    (ty::Ref(_, ty_from, from_mutbl), ty::Ref(_, ty_to, to_mutbl)) => {
                        if_chain! {
                            if let (&ty::Slice(slice_ty), &ty::Str) = (&ty_from.kind(), &ty_to.kind());
                            if let ty::Uint(ty::UintTy::U8) = slice_ty.kind();
                            if from_mutbl == to_mutbl;
                            then {
                                let postfix = if *from_mutbl == Mutability::Mut {
                                    "_mut"
                                } else {
                                    ""
                                };

                                span_lint_and_sugg(
                                    cx,
                                    TRANSMUTE_BYTES_TO_STR,
                                    e.span,
                                    &format!("transmute from a `{}` to a `{}`", from_ty, to_ty),
                                    "consider using",
                                    format!(
                                        "std::str::from_utf8{}({}).unwrap()",
                                        postfix,
                                        snippet(cx, args[0].span, ".."),
                                    ),
                                    Applicability::Unspecified,
                                );
                            } else {
                                if (cx.tcx.erase_regions(from_ty) != cx.tcx.erase_regions(to_ty))
                                    && !const_context {
                                    span_lint_and_then(
                                        cx,
                                        TRANSMUTE_PTR_TO_PTR,
                                        e.span,
                                        "transmute from a reference to a reference",
                                        |diag| if let Some(arg) = sugg::Sugg::hir_opt(cx, &args[0]) {
                                            let ty_from_and_mut = ty::TypeAndMut {
                                                ty: ty_from,
                                                mutbl: *from_mutbl
                                            };
                                            let ty_to_and_mut = ty::TypeAndMut { ty: ty_to, mutbl: *to_mutbl };
                                            let sugg_paren = arg
                                                .as_ty(cx.tcx.mk_ptr(ty_from_and_mut))
                                                .as_ty(cx.tcx.mk_ptr(ty_to_and_mut));
                                            let sugg = if *to_mutbl == Mutability::Mut {
                                                sugg_paren.mut_addr_deref()
                                            } else {
                                                sugg_paren.addr_deref()
                                            };
                                            diag.span_suggestion(
                                                e.span,
                                                "try",
                                                sugg.to_string(),
                                                Applicability::Unspecified,
                                            );
                                        },
                                    )
                                }
                            }
                        }
                    },
                    (ty::RawPtr(_), ty::RawPtr(to_ty)) => span_lint_and_then(
                        cx,
                        TRANSMUTE_PTR_TO_PTR,
                        e.span,
                        "transmute from a pointer to a pointer",
                        |diag| {
                            if let Some(arg) = sugg::Sugg::hir_opt(cx, &args[0]) {
                                let sugg = arg.as_ty(cx.tcx.mk_ptr(*to_ty));
                                diag.span_suggestion(e.span, "try", sugg.to_string(), Applicability::Unspecified);
                            }
                        },
                    ),
                    (ty::Int(ty::IntTy::I8) | ty::Uint(ty::UintTy::U8), ty::Bool) => {
                        span_lint_and_then(
                            cx,
                            TRANSMUTE_INT_TO_BOOL,
                            e.span,
                            &format!("transmute from a `{}` to a `bool`", from_ty),
                            |diag| {
                                let arg = sugg::Sugg::hir(cx, &args[0], "..");
                                let zero = sugg::Sugg::NonParen(Cow::from("0"));
                                diag.span_suggestion(
                                    e.span,
                                    "consider using",
                                    sugg::make_binop(ast::BinOpKind::Ne, &arg, &zero).to_string(),
                                    Applicability::Unspecified,
                                );
                            },
                        )
                    },
                    (ty::Int(_) | ty::Uint(_), ty::Float(_)) if !const_context => span_lint_and_then(
                        cx,
                        TRANSMUTE_INT_TO_FLOAT,
                        e.span,
                        &format!("transmute from a `{}` to a `{}`", from_ty, to_ty),
                        |diag| {
                            let arg = sugg::Sugg::hir(cx, &args[0], "..");
                            let arg = if let ty::Int(int_ty) = from_ty.kind() {
                                arg.as_ty(format!(
                                    "u{}",
                                    int_ty.bit_width().map_or_else(|| "size".to_string(), |v| v.to_string())
                                ))
                            } else {
                                arg
                            };
                            diag.span_suggestion(
                                e.span,
                                "consider using",
                                format!("{}::from_bits({})", to_ty, arg.to_string()),
                                Applicability::Unspecified,
                            );
                        },
                    ),
                    (ty::Float(float_ty), ty::Int(_) | ty::Uint(_)) if !const_context => span_lint_and_then(
                        cx,
                        TRANSMUTE_FLOAT_TO_INT,
                        e.span,
                        &format!("transmute from a `{}` to a `{}`", from_ty, to_ty),
                        |diag| {
                            let mut expr = &args[0];
                            let mut arg = sugg::Sugg::hir(cx, expr, "..");

                            if let ExprKind::Unary(UnOp::Neg, inner_expr) = &expr.kind {
                                expr = &inner_expr;
                            }

                            if_chain! {
                                // if the expression is a float literal and it is unsuffixed then
                                // add a suffix so the suggestion is valid and unambiguous
                                let op = format!("{}{}", arg, float_ty.name_str()).into();
                                if let ExprKind::Lit(lit) = &expr.kind;
                                if let ast::LitKind::Float(_, ast::LitFloatType::Unsuffixed) = lit.node;
                                then {
                                    match arg {
                                        sugg::Sugg::MaybeParen(_) => arg = sugg::Sugg::MaybeParen(op),
                                        _ => arg = sugg::Sugg::NonParen(op)
                                    }
                                }
                            }

                            arg = sugg::Sugg::NonParen(format!("{}.to_bits()", arg.maybe_par()).into());

                            // cast the result of `to_bits` if `to_ty` is signed
                            arg = if let ty::Int(int_ty) = to_ty.kind() {
                                arg.as_ty(int_ty.name_str().to_string())
                            } else {
                                arg
                            };

                            diag.span_suggestion(
                                e.span,
                                "consider using",
                                arg.to_string(),
                                Applicability::Unspecified,
                            );
                        },
                    ),
                    (ty::Adt(from_adt, from_substs), ty::Adt(to_adt, to_substs)) => {
                        if from_adt.did != to_adt.did ||
                                !COLLECTIONS.iter().any(|path| match_def_path(cx, to_adt.did, path)) {
                            return;
                        }
                        if from_substs.types().zip(to_substs.types())
                                              .any(|(from_ty, to_ty)| is_layout_incompatible(cx, from_ty, to_ty)) {
                            span_lint(
                                cx,
                                UNSOUND_COLLECTION_TRANSMUTE,
                                e.span,
                                &format!(
                                    "transmute from `{}` to `{}` with mismatched layout is unsound",
                                    from_ty,
                                    to_ty
                                )
                            );
                        }
                    },
                    (_, _) if can_be_expressed_as_pointer_cast(cx, e, from_ty, to_ty) => span_lint_and_then(
                        cx,
                        TRANSMUTES_EXPRESSIBLE_AS_PTR_CASTS,
                        e.span,
                        &format!(
                            "transmute from `{}` to `{}` which could be expressed as a pointer cast instead",
                            from_ty,
                            to_ty
                        ),
                        |diag| {
                            if let Some(arg) = sugg::Sugg::hir_opt(cx, &args[0]) {
                                let sugg = arg.as_ty(&to_ty.to_string()).to_string();
                                diag.span_suggestion(e.span, "try", sugg, Applicability::MachineApplicable);
                            }
                        }
                    ),
                    _ => {
                        return;
                    },
                }
            }
        }
    }
}
