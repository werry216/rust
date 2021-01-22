//! This file builds up the `ScopeTree`, which describes
//! the parent links in the region hierarchy.
//!
//! For more information about how MIR-based region-checking works,
//! see the [rustc dev guide].
//!
//! [rustc dev guide]: https://rustc-dev-guide.rust-lang.org/borrow_check.html

use rustc_ast::walk_list;
use rustc_data_structures::fx::FxHashSet;
use rustc_hir as hir;
use rustc_hir::def_id::DefId;
use rustc_hir::intravisit::{self, NestedVisitorMap, Visitor};
use rustc_hir::{Arm, Block, Expr, Local, Node, Pat, PatKind, Stmt};
use rustc_index::vec::Idx;
use rustc_middle::middle::region::*;
use rustc_middle::ty::query::Providers;
use rustc_middle::ty::TyCtxt;
use rustc_span::source_map;
use rustc_span::Span;

use std::mem;

#[derive(Debug, Copy, Clone)]
pub struct Context {
    /// The root of the current region tree. This is typically the id
    /// of the innermost fn body. Each fn forms its own disjoint tree
    /// in the region hierarchy. These fn bodies are themselves
    /// arranged into a tree. See the "Modeling closures" section of
    /// the README in `rustc_trait_selection::infer::region_constraints`
    /// for more details.
    root_id: Option<hir::ItemLocalId>,

    /// The scope that contains any new variables declared, plus its depth in
    /// the scope tree.
    var_parent: Option<(Scope, ScopeDepth)>,

    /// Region parent of expressions, etc., plus its depth in the scope tree.
    parent: Option<(Scope, ScopeDepth)>,
}

struct RegionResolutionVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,

    // The number of expressions and patterns visited in the current body.
    expr_and_pat_count: usize,
    // When this is `true`, we record the `Scopes` we encounter
    // when processing a Yield expression. This allows us to fix
    // up their indices.
    pessimistic_yield: bool,
    // Stores scopes when `pessimistic_yield` is `true`.
    fixup_scopes: Vec<Scope>,
    // The generated scope tree.
    scope_tree: ScopeTree,

    cx: Context,

    /// `terminating_scopes` is a set containing the ids of each
    /// statement, or conditional/repeating expression. These scopes
    /// are calling "terminating scopes" because, when attempting to
    /// find the scope of a temporary, by default we search up the
    /// enclosing scopes until we encounter the terminating scope. A
    /// conditional/repeating expression is one which is not
    /// guaranteed to execute exactly once upon entering the parent
    /// scope. This could be because the expression only executes
    /// conditionally, such as the expression `b` in `a && b`, or
    /// because the expression may execute many times, such as a loop
    /// body. The reason that we distinguish such expressions is that,
    /// upon exiting the parent scope, we cannot statically know how
    /// many times the expression executed, and thus if the expression
    /// creates temporaries we cannot know statically how many such
    /// temporaries we would have to cleanup. Therefore, we ensure that
    /// the temporaries never outlast the conditional/repeating
    /// expression, preventing the need for dynamic checks and/or
    /// arbitrary amounts of stack space. Terminating scopes end
    /// up being contained in a DestructionScope that contains the
    /// destructor's execution.
    terminating_scopes: FxHashSet<hir::ItemLocalId>,
}

/// Records the lifetime of a local variable as `cx.var_parent`
fn record_var_lifetime(
    visitor: &mut RegionResolutionVisitor<'_>,
    var_id: hir::ItemLocalId,
    _sp: Span,
) {
    match visitor.cx.var_parent {
        None => {
            // this can happen in extern fn declarations like
            //
            // extern fn isalnum(c: c_int) -> c_int
        }
        Some((parent_scope, _)) => visitor.scope_tree.record_var_scope(var_id, parent_scope),
    }
}

fn resolve_block<'tcx>(visitor: &mut RegionResolutionVisitor<'tcx>, blk: &'tcx hir::Block<'tcx>) {
    debug!("resolve_block(blk.hir_id={:?})", blk.hir_id);

    let prev_cx = visitor.cx;

    // We treat the tail expression in the block (if any) somewhat
    // differently from the statements. The issue has to do with
    // temporary lifetimes. Consider the following:
    //
    //    quux({
    //        let inner = ... (&bar()) ...;
    //
    //        (... (&foo()) ...) // (the tail expression)
    //    }, other_argument());
    //
    // Each of the statements within the block is a terminating
    // scope, and thus a temporary (e.g., the result of calling
    // `bar()` in the initializer expression for `let inner = ...;`)
    // will be cleaned up immediately after its corresponding
    // statement (i.e., `let inner = ...;`) executes.
    //
    // On the other hand, temporaries associated with evaluating the
    // tail expression for the block are assigned lifetimes so that
    // they will be cleaned up as part of the terminating scope
    // *surrounding* the block expression. Here, the terminating
    // scope for the block expression is the `quux(..)` call; so
    // those temporaries will only be cleaned up *after* both
    // `other_argument()` has run and also the call to `quux(..)`
    // itself has returned.

    visitor.enter_node_scope_with_dtor(blk.hir_id.local_id);
    visitor.cx.var_parent = visitor.cx.parent;

    {
        // This block should be kept approximately in sync with
        // `intravisit::walk_block`. (We manually walk the block, rather
        // than call `walk_block`, in order to maintain precise
        // index information.)

        for (i, statement) in blk.stmts.iter().enumerate() {
            match statement.kind {
                hir::StmtKind::Local(..) | hir::StmtKind::Item(..) => {
                    // Each declaration introduces a subscope for bindings
                    // introduced by the declaration; this subscope covers a
                    // suffix of the block. Each subscope in a block has the
                    // previous subscope in the block as a parent, except for
                    // the first such subscope, which has the block itself as a
                    // parent.
                    visitor.enter_scope(Scope {
                        id: blk.hir_id.local_id,
                        data: ScopeData::Remainder(FirstStatementIndex::new(i)),
                    });
                    visitor.cx.var_parent = visitor.cx.parent;
                }
                hir::StmtKind::Expr(..) | hir::StmtKind::Semi(..) => {}
            }
            visitor.visit_stmt(statement)
        }
        walk_list!(visitor, visit_expr, &blk.expr);
    }

    visitor.cx = prev_cx;
}

fn resolve_arm<'tcx>(visitor: &mut RegionResolutionVisitor<'tcx>, arm: &'tcx hir::Arm<'tcx>) {
    let prev_cx = visitor.cx;

    visitor.enter_scope(Scope { id: arm.hir_id.local_id, data: ScopeData::Node });
    visitor.cx.var_parent = visitor.cx.parent;

    visitor.terminating_scopes.insert(arm.body.hir_id.local_id);

    if let Some(hir::Guard::If(ref expr)) = arm.guard {
        visitor.terminating_scopes.insert(expr.hir_id.local_id);
    }

    intravisit::walk_arm(visitor, arm);

    visitor.cx = prev_cx;
}

fn resolve_pat<'tcx>(visitor: &mut RegionResolutionVisitor<'tcx>, pat: &'tcx hir::Pat<'tcx>) {
    visitor.record_child_scope(Scope { id: pat.hir_id.local_id, data: ScopeData::Node });

    // If this is a binding then record the lifetime of that binding.
    if let PatKind::Binding(..) = pat.kind {
        record_var_lifetime(visitor, pat.hir_id.local_id, pat.span);
    }

    debug!("resolve_pat - pre-increment {} pat = {:?}", visitor.expr_and_pat_count, pat);

    intravisit::walk_pat(visitor, pat);

    visitor.expr_and_pat_count += 1;

    debug!("resolve_pat - post-increment {} pat = {:?}", visitor.expr_and_pat_count, pat);
}

fn resolve_stmt<'tcx>(visitor: &mut RegionResolutionVisitor<'tcx>, stmt: &'tcx hir::Stmt<'tcx>) {
    let stmt_id = stmt.hir_id.local_id;
    debug!("resolve_stmt(stmt.id={:?})", stmt_id);

    // Every statement will clean up the temporaries created during
    // execution of that statement. Therefore each statement has an
    // associated destruction scope that represents the scope of the
    // statement plus its destructors, and thus the scope for which
    // regions referenced by the destructors need to survive.
    visitor.terminating_scopes.insert(stmt_id);

    let prev_parent = visitor.cx.parent;
    visitor.enter_node_scope_with_dtor(stmt_id);

    intravisit::walk_stmt(visitor, stmt);

    visitor.cx.parent = prev_parent;
}

fn resolve_expr<'tcx>(visitor: &mut RegionResolutionVisitor<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
    debug!("resolve_expr - pre-increment {} expr = {:?}", visitor.expr_and_pat_count, expr);

    let prev_cx = visitor.cx;
    visitor.enter_node_scope_with_dtor(expr.hir_id.local_id);

    {
        let terminating_scopes = &mut visitor.terminating_scopes;
        let mut terminating = |id: hir::ItemLocalId| {
            terminating_scopes.insert(id);
        };
        match expr.kind {
            // Conditional or repeating scopes are always terminating
            // scopes, meaning that temporaries cannot outlive them.
            // This ensures fixed size stacks.
            hir::ExprKind::Binary(
                source_map::Spanned { node: hir::BinOpKind::And, .. },
                _,
                ref r,
            )
            | hir::ExprKind::Binary(
                source_map::Spanned { node: hir::BinOpKind::Or, .. },
                _,
                ref r,
            ) => {
                // For shortcircuiting operators, mark the RHS as a terminating
                // scope since it only executes conditionally.
                terminating(r.hir_id.local_id);
            }

            hir::ExprKind::If(ref expr, ref then, Some(ref otherwise)) => {
                terminating(expr.hir_id.local_id);
                terminating(then.hir_id.local_id);
                terminating(otherwise.hir_id.local_id);
            }

            hir::ExprKind::If(ref expr, ref then, None) => {
                terminating(expr.hir_id.local_id);
                terminating(then.hir_id.local_id);
            }

            hir::ExprKind::Loop(ref body, _, _, _) => {
                terminating(body.hir_id.local_id);
            }

            hir::ExprKind::DropTemps(ref expr) => {
                // `DropTemps(expr)` does not denote a conditional scope.
                // Rather, we want to achieve the same behavior as `{ let _t = expr; _t }`.
                terminating(expr.hir_id.local_id);
            }

            hir::ExprKind::AssignOp(..)
            | hir::ExprKind::Index(..)
            | hir::ExprKind::Unary(..)
            | hir::ExprKind::Call(..)
            | hir::ExprKind::MethodCall(..) => {
                // FIXME(https://github.com/rust-lang/rfcs/issues/811) Nested method calls
                //
                // The lifetimes for a call or method call look as follows:
                //
                // call.id
                // - arg0.id
                // - ...
                // - argN.id
                // - call.callee_id
                //
                // The idea is that call.callee_id represents *the time when
                // the invoked function is actually running* and call.id
                // represents *the time to prepare the arguments and make the
                // call*.  See the section "Borrows in Calls" borrowck/README.md
                // for an extended explanation of why this distinction is
                // important.
                //
                // record_superlifetime(new_cx, expr.callee_id);
            }

            _ => {}
        }
    }

    let prev_pessimistic = visitor.pessimistic_yield;

    // Ordinarily, we can rely on the visit order of HIR intravisit
    // to correspond to the actual execution order of statements.
    // However, there's a weird corner case with compound assignment
    // operators (e.g. `a += b`). The evaluation order depends on whether
    // or not the operator is overloaded (e.g. whether or not a trait
    // like AddAssign is implemented).

    // For primitive types (which, despite having a trait impl, don't actually
    // end up calling it), the evluation order is right-to-left. For example,
    // the following code snippet:
    //
    //    let y = &mut 0;
    //    *{println!("LHS!"); y} += {println!("RHS!"); 1};
    //
    // will print:
    //
    // RHS!
    // LHS!
    //
    // However, if the operator is used on a non-primitive type,
    // the evaluation order will be left-to-right, since the operator
    // actually get desugared to a method call. For example, this
    // nearly identical code snippet:
    //
    //     let y = &mut String::new();
    //    *{println!("LHS String"); y} += {println!("RHS String"); "hi"};
    //
    // will print:
    // LHS String
    // RHS String
    //
    // To determine the actual execution order, we need to perform
    // trait resolution. Unfortunately, we need to be able to compute
    // yield_in_scope before type checking is even done, as it gets
    // used by AST borrowcheck.
    //
    // Fortunately, we don't need to know the actual execution order.
    // It suffices to know the 'worst case' order with respect to yields.
    // Specifically, we need to know the highest 'expr_and_pat_count'
    // that we could assign to the yield expression. To do this,
    // we pick the greater of the two values from the left-hand
    // and right-hand expressions. This makes us overly conservative
    // about what types could possibly live across yield points,
    // but we will never fail to detect that a type does actually
    // live across a yield point. The latter part is critical -
    // we're already overly conservative about what types will live
    // across yield points, as the generated MIR will determine
    // when things are actually live. However, for typecheck to work
    // properly, we can't miss any types.

    match expr.kind {
        // Manually recurse over closures, because they are the only
        // case of nested bodies that share the parent environment.
        hir::ExprKind::Closure(.., body, _, _) => {
            let body = visitor.tcx.hir().body(body);
            visitor.visit_body(body);
        }
        hir::ExprKind::AssignOp(_, ref left_expr, ref right_expr) => {
            debug!(
                "resolve_expr - enabling pessimistic_yield, was previously {}",
                prev_pessimistic
            );

            let start_point = visitor.fixup_scopes.len();
            visitor.pessimistic_yield = true;

            // If the actual execution order turns out to be right-to-left,
            // then we're fine. However, if the actual execution order is left-to-right,
            // then we'll assign too low a count to any `yield` expressions
            // we encounter in 'right_expression' - they should really occur after all of the
            // expressions in 'left_expression'.
            visitor.visit_expr(&right_expr);
            visitor.pessimistic_yield = prev_pessimistic;

            debug!("resolve_expr - restoring pessimistic_yield to {}", prev_pessimistic);
            visitor.visit_expr(&left_expr);
            debug!("resolve_expr - fixing up counts to {}", visitor.expr_and_pat_count);

            // Remove and process any scopes pushed by the visitor
            let target_scopes = visitor.fixup_scopes.drain(start_point..);

            for scope in target_scopes {
                let mut yield_data = visitor.scope_tree.yield_in_scope.get_mut(&scope).unwrap();
                let count = yield_data.expr_and_pat_count;
                let span = yield_data.span;

                // expr_and_pat_count never decreases. Since we recorded counts in yield_in_scope
                // before walking the left-hand side, it should be impossible for the recorded
                // count to be greater than the left-hand side count.
                if count > visitor.expr_and_pat_count {
                    bug!(
                        "Encountered greater count {} at span {:?} - expected no greater than {}",
                        count,
                        span,
                        visitor.expr_and_pat_count
                    );
                }
                let new_count = visitor.expr_and_pat_count;
                debug!(
                    "resolve_expr - increasing count for scope {:?} from {} to {} at span {:?}",
                    scope, count, new_count, span
                );

                yield_data.expr_and_pat_count = new_count;
            }
        }

        _ => intravisit::walk_expr(visitor, expr),
    }

    visitor.expr_and_pat_count += 1;

    debug!("resolve_expr post-increment {}, expr = {:?}", visitor.expr_and_pat_count, expr);

    if let hir::ExprKind::Yield(_, source) = &expr.kind {
        // Mark this expr's scope and all parent scopes as containing `yield`.
        let mut scope = Scope { id: expr.hir_id.local_id, data: ScopeData::Node };
        loop {
            let data = YieldData {
                span: expr.span,
                expr_and_pat_count: visitor.expr_and_pat_count,
                source: *source,
            };
            visitor.scope_tree.yield_in_scope.insert(scope, data);
            if visitor.pessimistic_yield {
                debug!("resolve_expr in pessimistic_yield - marking scope {:?} for fixup", scope);
                visitor.fixup_scopes.push(scope);
            }

            // Keep traversing up while we can.
            match visitor.scope_tree.parent_map.get(&scope) {
                // Don't cross from closure bodies to their parent.
                Some(&(superscope, _)) => match superscope.data {
                    ScopeData::CallSite => break,
                    _ => scope = superscope,
                },
                None => break,
            }
        }
    }

    visitor.cx = prev_cx;
}

fn resolve_local<'tcx>(
    visitor: &mut RegionResolutionVisitor<'tcx>,
    pat: Option<&'tcx hir::Pat<'tcx>>,
    init: Option<&'tcx hir::Expr<'tcx>>,
) {
    debug!("resolve_local(pat={:?}, init={:?})", pat, init);

    let blk_scope = visitor.cx.var_parent.map(|(p, _)| p);

    // As an exception to the normal rules governing temporary
    // lifetimes, initializers in a let have a temporary lifetime
    // of the enclosing block. This means that e.g., a program
    // like the following is legal:
    //
    //     let ref x = HashMap::new();
    //
    // Because the hash map will be freed in the enclosing block.
    //
    // We express the rules more formally based on 3 grammars (defined
    // fully in the helpers below that implement them):
    //
    // 1. `E&`, which matches expressions like `&<rvalue>` that
    //    own a pointer into the stack.
    //
    // 2. `P&`, which matches patterns like `ref x` or `(ref x, ref
    //    y)` that produce ref bindings into the value they are
    //    matched against or something (at least partially) owned by
    //    the value they are matched against. (By partially owned,
    //    I mean that creating a binding into a ref-counted or managed value
    //    would still count.)
    //
    // 3. `ET`, which matches both rvalues like `foo()` as well as places
    //    based on rvalues like `foo().x[2].y`.
    //
    // A subexpression `<rvalue>` that appears in a let initializer
    // `let pat [: ty] = expr` has an extended temporary lifetime if
    // any of the following conditions are met:
    //
    // A. `pat` matches `P&` and `expr` matches `ET`
    //    (covers cases where `pat` creates ref bindings into an rvalue
    //     produced by `expr`)
    // B. `ty` is a borrowed pointer and `expr` matches `ET`
    //    (covers cases where coercion creates a borrow)
    // C. `expr` matches `E&`
    //    (covers cases `expr` borrows an rvalue that is then assigned
    //     to memory (at least partially) owned by the binding)
    //
    // Here are some examples hopefully giving an intuition where each
    // rule comes into play and why:
    //
    // Rule A. `let (ref x, ref y) = (foo().x, 44)`. The rvalue `(22, 44)`
    // would have an extended lifetime, but not `foo()`.
    //
    // Rule B. `let x = &foo().x`. The rvalue `foo()` would have extended
    // lifetime.
    //
    // In some cases, multiple rules may apply (though not to the same
    // rvalue). For example:
    //
    //     let ref x = [&a(), &b()];
    //
    // Here, the expression `[...]` has an extended lifetime due to rule
    // A, but the inner rvalues `a()` and `b()` have an extended lifetime
    // due to rule C.

    if let Some(expr) = init {
        record_rvalue_scope_if_borrow_expr(visitor, &expr, blk_scope);

        if let Some(pat) = pat {
            if is_binding_pat(pat) {
                record_rvalue_scope(visitor, &expr, blk_scope);
            }
        }
    }

    // Make sure we visit the initializer first, so expr_and_pat_count remains correct
    if let Some(expr) = init {
        visitor.visit_expr(expr);
    }
    if let Some(pat) = pat {
        visitor.visit_pat(pat);
    }

    /// Returns `true` if `pat` match the `P&` non-terminal.
    ///
    /// ```text
    ///     P& = ref X
    ///        | StructName { ..., P&, ... }
    ///        | VariantName(..., P&, ...)
    ///        | [ ..., P&, ... ]
    ///        | ( ..., P&, ... )
    ///        | ... "|" P& "|" ...
    ///        | box P&
    /// ```
    fn is_binding_pat(pat: &hir::Pat<'_>) -> bool {
        // Note that the code below looks for *explicit* refs only, that is, it won't
        // know about *implicit* refs as introduced in #42640.
        //
        // This is not a problem. For example, consider
        //
        //      let (ref x, ref y) = (Foo { .. }, Bar { .. });
        //
        // Due to the explicit refs on the left hand side, the below code would signal
        // that the temporary value on the right hand side should live until the end of
        // the enclosing block (as opposed to being dropped after the let is complete).
        //
        // To create an implicit ref, however, you must have a borrowed value on the RHS
        // already, as in this example (which won't compile before #42640):
        //
        //      let Foo { x, .. } = &Foo { x: ..., ... };
        //
        // in place of
        //
        //      let Foo { ref x, .. } = Foo { ... };
        //
        // In the former case (the implicit ref version), the temporary is created by the
        // & expression, and its lifetime would be extended to the end of the block (due
        // to a different rule, not the below code).
        match pat.kind {
            PatKind::Binding(hir::BindingAnnotation::Ref, ..)
            | PatKind::Binding(hir::BindingAnnotation::RefMut, ..) => true,

            PatKind::Struct(_, ref field_pats, _) => {
                field_pats.iter().any(|fp| is_binding_pat(&fp.pat))
            }

            PatKind::Slice(ref pats1, ref pats2, ref pats3) => {
                pats1.iter().any(|p| is_binding_pat(&p))
                    || pats2.iter().any(|p| is_binding_pat(&p))
                    || pats3.iter().any(|p| is_binding_pat(&p))
            }

            PatKind::Or(ref subpats)
            | PatKind::TupleStruct(_, ref subpats, _)
            | PatKind::Tuple(ref subpats, _) => subpats.iter().any(|p| is_binding_pat(&p)),

            PatKind::Box(ref subpat) => is_binding_pat(&subpat),

            PatKind::Ref(_, _)
            | PatKind::Binding(
                hir::BindingAnnotation::Unannotated | hir::BindingAnnotation::Mutable,
                ..,
            )
            | PatKind::Wild
            | PatKind::Path(_)
            | PatKind::Lit(_)
            | PatKind::Range(_, _, _) => false,
        }
    }

    /// If `expr` matches the `E&` grammar, then records an extended rvalue scope as appropriate:
    ///
    /// ```text
    ///     E& = & ET
    ///        | StructName { ..., f: E&, ... }
    ///        | [ ..., E&, ... ]
    ///        | ( ..., E&, ... )
    ///        | {...; E&}
    ///        | box E&
    ///        | E& as ...
    ///        | ( E& )
    /// ```
    fn record_rvalue_scope_if_borrow_expr<'tcx>(
        visitor: &mut RegionResolutionVisitor<'tcx>,
        expr: &hir::Expr<'_>,
        blk_id: Option<Scope>,
    ) {
        match expr.kind {
            hir::ExprKind::AddrOf(_, _, ref subexpr) => {
                record_rvalue_scope_if_borrow_expr(visitor, &subexpr, blk_id);
                record_rvalue_scope(visitor, &subexpr, blk_id);
            }
            hir::ExprKind::Struct(_, fields, _) => {
                for field in fields {
                    record_rvalue_scope_if_borrow_expr(visitor, &field.expr, blk_id);
                }
            }
            hir::ExprKind::Array(subexprs) | hir::ExprKind::Tup(subexprs) => {
                for subexpr in subexprs {
                    record_rvalue_scope_if_borrow_expr(visitor, &subexpr, blk_id);
                }
            }
            hir::ExprKind::Cast(ref subexpr, _) => {
                record_rvalue_scope_if_borrow_expr(visitor, &subexpr, blk_id)
            }
            hir::ExprKind::Block(ref block, _) => {
                if let Some(ref subexpr) = block.expr {
                    record_rvalue_scope_if_borrow_expr(visitor, &subexpr, blk_id);
                }
            }
            _ => {}
        }
    }

    /// Applied to an expression `expr` if `expr` -- or something owned or partially owned by
    /// `expr` -- is going to be indirectly referenced by a variable in a let statement. In that
    /// case, the "temporary lifetime" or `expr` is extended to be the block enclosing the `let`
    /// statement.
    ///
    /// More formally, if `expr` matches the grammar `ET`, record the rvalue scope of the matching
    /// `<rvalue>` as `blk_id`:
    ///
    /// ```text
    ///     ET = *ET
    ///        | ET[...]
    ///        | ET.f
    ///        | (ET)
    ///        | <rvalue>
    /// ```
    ///
    /// Note: ET is intended to match "rvalues or places based on rvalues".
    fn record_rvalue_scope<'tcx>(
        visitor: &mut RegionResolutionVisitor<'tcx>,
        expr: &hir::Expr<'_>,
        blk_scope: Option<Scope>,
    ) {
        let mut expr = expr;
        loop {
            // Note: give all the expressions matching `ET` with the
            // extended temporary lifetime, not just the innermost rvalue,
            // because in codegen if we must compile e.g., `*rvalue()`
            // into a temporary, we request the temporary scope of the
            // outer expression.
            visitor.scope_tree.record_rvalue_scope(expr.hir_id.local_id, blk_scope);

            match expr.kind {
                hir::ExprKind::AddrOf(_, _, ref subexpr)
                | hir::ExprKind::Unary(hir::UnOp::UnDeref, ref subexpr)
                | hir::ExprKind::Field(ref subexpr, _)
                | hir::ExprKind::Index(ref subexpr, _) => {
                    expr = &subexpr;
                }
                _ => {
                    return;
                }
            }
        }
    }
}

impl<'tcx> RegionResolutionVisitor<'tcx> {
    /// Records the current parent (if any) as the parent of `child_scope`.
    /// Returns the depth of `child_scope`.
    fn record_child_scope(&mut self, child_scope: Scope) -> ScopeDepth {
        let parent = self.cx.parent;
        self.scope_tree.record_scope_parent(child_scope, parent);
        // If `child_scope` has no parent, it must be the root node, and so has
        // a depth of 1. Otherwise, its depth is one more than its parent's.
        parent.map_or(1, |(_p, d)| d + 1)
    }

    /// Records the current parent (if any) as the parent of `child_scope`,
    /// and sets `child_scope` as the new current parent.
    fn enter_scope(&mut self, child_scope: Scope) {
        let child_depth = self.record_child_scope(child_scope);
        self.cx.parent = Some((child_scope, child_depth));
    }

    fn enter_node_scope_with_dtor(&mut self, id: hir::ItemLocalId) {
        // If node was previously marked as a terminating scope during the
        // recursive visit of its parent node in the AST, then we need to
        // account for the destruction scope representing the scope of
        // the destructors that run immediately after it completes.
        if self.terminating_scopes.contains(&id) {
            self.enter_scope(Scope { id, data: ScopeData::Destruction });
        }
        self.enter_scope(Scope { id, data: ScopeData::Node });
    }
}

impl<'tcx> Visitor<'tcx> for RegionResolutionVisitor<'tcx> {
    type Map = intravisit::ErasedMap<'tcx>;

    fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
        NestedVisitorMap::None
    }

    fn visit_block(&mut self, b: &'tcx Block<'tcx>) {
        resolve_block(self, b);
    }

    fn visit_body(&mut self, body: &'tcx hir::Body<'tcx>) {
        let body_id = body.id();
        let owner_id = self.tcx.hir().body_owner(body_id);

        debug!(
            "visit_body(id={:?}, span={:?}, body.id={:?}, cx.parent={:?})",
            owner_id,
            self.tcx.sess.source_map().span_to_string(body.value.span),
            body_id,
            self.cx.parent
        );

        // Save all state that is specific to the outer function
        // body. These will be restored once down below, once we've
        // visited the body.
        let outer_ec = mem::replace(&mut self.expr_and_pat_count, 0);
        let outer_cx = self.cx;
        let outer_ts = mem::take(&mut self.terminating_scopes);
        // The 'pessimistic yield' flag is set to true when we are
        // processing a `+=` statement and have to make pessimistic
        // control flow assumptions. This doesn't apply to nested
        // bodies within the `+=` statements. See #69307.
        let outer_pessimistic_yield = mem::replace(&mut self.pessimistic_yield, false);
        self.terminating_scopes.insert(body.value.hir_id.local_id);

        if let Some(root_id) = self.cx.root_id {
            self.scope_tree.record_closure_parent(body.value.hir_id.local_id, root_id);
        }
        self.cx.root_id = Some(body.value.hir_id.local_id);

        self.enter_scope(Scope { id: body.value.hir_id.local_id, data: ScopeData::CallSite });
        self.enter_scope(Scope { id: body.value.hir_id.local_id, data: ScopeData::Arguments });

        // The arguments and `self` are parented to the fn.
        self.cx.var_parent = self.cx.parent.take();
        for param in body.params {
            self.visit_pat(&param.pat);
        }

        // The body of the every fn is a root scope.
        self.cx.parent = self.cx.var_parent;
        if self.tcx.hir().body_owner_kind(owner_id).is_fn_or_closure() {
            self.visit_expr(&body.value)
        } else {
            // Only functions have an outer terminating (drop) scope, while
            // temporaries in constant initializers may be 'static, but only
            // according to rvalue lifetime semantics, using the same
            // syntactical rules used for let initializers.
            //
            // e.g., in `let x = &f();`, the temporary holding the result from
            // the `f()` call lives for the entirety of the surrounding block.
            //
            // Similarly, `const X: ... = &f();` would have the result of `f()`
            // live for `'static`, implying (if Drop restrictions on constants
            // ever get lifted) that the value *could* have a destructor, but
            // it'd get leaked instead of the destructor running during the
            // evaluation of `X` (if at all allowed by CTFE).
            //
            // However, `const Y: ... = g(&f());`, like `let y = g(&f());`,
            // would *not* let the `f()` temporary escape into an outer scope
            // (i.e., `'static`), which means that after `g` returns, it drops,
            // and all the associated destruction scope rules apply.
            self.cx.var_parent = None;
            resolve_local(self, None, Some(&body.value));
        }

        if body.generator_kind.is_some() {
            self.scope_tree.body_expr_count.insert(body_id, self.expr_and_pat_count);
        }

        // Restore context we had at the start.
        self.expr_and_pat_count = outer_ec;
        self.cx = outer_cx;
        self.terminating_scopes = outer_ts;
        self.pessimistic_yield = outer_pessimistic_yield;
    }

    fn visit_arm(&mut self, a: &'tcx Arm<'tcx>) {
        resolve_arm(self, a);
    }
    fn visit_pat(&mut self, p: &'tcx Pat<'tcx>) {
        resolve_pat(self, p);
    }
    fn visit_stmt(&mut self, s: &'tcx Stmt<'tcx>) {
        resolve_stmt(self, s);
    }
    fn visit_expr(&mut self, ex: &'tcx Expr<'tcx>) {
        resolve_expr(self, ex);
    }
    fn visit_local(&mut self, l: &'tcx Local<'tcx>) {
        resolve_local(self, Some(&l.pat), l.init.as_deref());
    }
}

fn region_scope_tree(tcx: TyCtxt<'_>, def_id: DefId) -> &ScopeTree {
    let closure_base_def_id = tcx.closure_base_def_id(def_id);
    if closure_base_def_id != def_id {
        return tcx.region_scope_tree(closure_base_def_id);
    }

    let id = tcx.hir().local_def_id_to_hir_id(def_id.expect_local());
    let scope_tree = if let Some(body_id) = tcx.hir().maybe_body_owned_by(id) {
        let mut visitor = RegionResolutionVisitor {
            tcx,
            scope_tree: ScopeTree::default(),
            expr_and_pat_count: 0,
            cx: Context { root_id: None, parent: None, var_parent: None },
            terminating_scopes: Default::default(),
            pessimistic_yield: false,
            fixup_scopes: vec![],
        };

        let body = tcx.hir().body(body_id);
        visitor.scope_tree.root_body = Some(body.value.hir_id);

        // If the item is an associated const or a method,
        // record its impl/trait parent, as it can also have
        // lifetime parameters free in this body.
        match tcx.hir().get(id) {
            Node::ImplItem(_) | Node::TraitItem(_) => {
                visitor.scope_tree.root_parent = Some(tcx.hir().get_parent_item(id));
            }
            _ => {}
        }

        visitor.visit_body(body);

        visitor.scope_tree
    } else {
        ScopeTree::default()
    };

    tcx.arena.alloc(scope_tree)
}

pub fn provide(providers: &mut Providers) {
    *providers = Providers { region_scope_tree, ..*providers };
}
