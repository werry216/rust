//! See docs in build/expr/mod.rs

use crate::build::expr::category::{Category, RvalueFunc};
use crate::build::scope::DropKind;
use crate::build::{BlockAnd, BlockAndExtension, BlockFrame, Builder};
use crate::thir::*;
use rustc_ast::InlineAsmOptions;
use rustc_data_structures::fx::FxHashMap;
use rustc_data_structures::stack::ensure_sufficient_stack;
use rustc_hir as hir;
use rustc_middle::middle::region;
use rustc_middle::mir::*;
use rustc_middle::ty::CanonicalUserTypeAnnotation;

use std::slice;

impl<'a, 'tcx> Builder<'a, 'tcx> {
    /// Compile `expr`, storing the result into `destination`, which
    /// is assumed to be uninitialized.
    /// If a `drop_scope` is provided, `destination` is scheduled to be dropped
    /// in `scope` once it has been initialized.
    crate fn into_expr(
        &mut self,
        destination: Place<'tcx>,
        scope: Option<region::Scope>,
        mut block: BasicBlock,
        expr: Expr<'tcx>,
    ) -> BlockAnd<()> {
        debug!(
            "into_expr(destination={:?}, scope={:?}, block={:?}, expr={:?})",
            destination, scope, block, expr
        );

        // since we frequently have to reference `self` from within a
        // closure, where `self` would be shadowed, it's easier to
        // just use the name `this` uniformly
        let this = self;
        let expr_span = expr.span;
        let source_info = this.source_info(expr_span);

        let expr_is_block_or_scope =
            matches!(expr.kind, ExprKind::Block { .. } | ExprKind::Scope { .. });

        let schedule_drop = move |this: &mut Self| {
            if let Some(drop_scope) = scope {
                let local =
                    destination.as_local().expect("cannot schedule drop of non-Local place");
                this.schedule_drop(expr_span, drop_scope, local, DropKind::Value);
            }
        };

        if !expr_is_block_or_scope {
            this.block_context.push(BlockFrame::SubExpr);
        }

        let block_and = match expr.kind {
            ExprKind::Scope { region_scope, lint_level, value } => {
                let region_scope = (region_scope, source_info);
                ensure_sufficient_stack(|| {
                    this.in_scope(region_scope, lint_level, |this| {
                        this.into(destination, scope, block, value)
                    })
                })
            }
            ExprKind::Block { body: ast_block } => {
                this.ast_block(destination, scope, block, ast_block, source_info)
            }
            ExprKind::Match { scrutinee, arms } => {
                this.match_expr(destination, scope, expr_span, block, scrutinee, arms)
            }
            ExprKind::If { cond, then, else_opt } => {
                let place = unpack!(
                    block = this.as_temp(block, Some(this.local_scope()), cond, Mutability::Mut)
                );
                let operand = Operand::Move(Place::from(place));

                let mut then_block = this.cfg.start_new_block();
                let mut else_block = this.cfg.start_new_block();
                let term = TerminatorKind::if_(this.hir.tcx(), operand, then_block, else_block);
                this.cfg.terminate(block, source_info, term);

                unpack!(then_block = this.into(destination, scope, then_block, then));
                else_block = if let Some(else_opt) = else_opt {
                    unpack!(this.into(destination, None, else_block, else_opt))
                } else {
                    // Body of the `if` expression without an `else` clause must return `()`, thus
                    // we implicitly generate a `else {}` if it is not specified.
                    let correct_si = this.source_info(expr_span.shrink_to_hi());
                    this.cfg.push_assign_unit(else_block, correct_si, destination, this.hir.tcx());
                    else_block
                };

                let join_block = this.cfg.start_new_block();
                this.cfg.terminate(
                    then_block,
                    source_info,
                    TerminatorKind::Goto { target: join_block },
                );
                this.cfg.terminate(
                    else_block,
                    source_info,
                    TerminatorKind::Goto { target: join_block },
                );

                join_block.unit()
            }
            ExprKind::NeverToAny { source } => {
                let source = this.hir.mirror(source);
                let is_call =
                    matches!(source.kind, ExprKind::Call { .. } | ExprKind::InlineAsm { .. });

                // (#66975) Source could be a const of type `!`, so has to
                // exist in the generated MIR.
                unpack!(
                    block = this.as_temp(block, Some(this.local_scope()), source, Mutability::Mut,)
                );

                // This is an optimization. If the expression was a call then we already have an
                // unreachable block. Don't bother to terminate it and create a new one.
                schedule_drop(this);
                if is_call {
                    block.unit()
                } else {
                    this.cfg.terminate(block, source_info, TerminatorKind::Unreachable);
                    let end_block = this.cfg.start_new_block();
                    end_block.unit()
                }
            }
            ExprKind::LogicalOp { op, lhs, rhs } => {
                // And:
                //
                // [block: If(lhs)] -true-> [else_block: If(rhs)] -true-> [true_block]
                //        |                          | (false)
                //        +----------false-----------+------------------> [false_block]
                //
                // Or:
                //
                // [block: If(lhs)] -false-> [else_block: If(rhs)] -true-> [true_block]
                //        | (true)                   | (false)
                //  [true_block]               [false_block]

                let (true_block, false_block, mut else_block, join_block) = (
                    this.cfg.start_new_block(),
                    this.cfg.start_new_block(),
                    this.cfg.start_new_block(),
                    this.cfg.start_new_block(),
                );

                let lhs = unpack!(block = this.as_local_operand(block, lhs));
                let blocks = match op {
                    LogicalOp::And => (else_block, false_block),
                    LogicalOp::Or => (true_block, else_block),
                };
                let term = TerminatorKind::if_(this.hir.tcx(), lhs, blocks.0, blocks.1);
                this.cfg.terminate(block, source_info, term);

                let rhs = unpack!(else_block = this.as_local_operand(else_block, rhs));
                let term = TerminatorKind::if_(this.hir.tcx(), rhs, true_block, false_block);
                this.cfg.terminate(else_block, source_info, term);

                this.cfg.push_assign_constant(
                    true_block,
                    source_info,
                    destination,
                    Constant { span: expr_span, user_ty: None, literal: this.hir.true_literal() },
                );

                this.cfg.push_assign_constant(
                    false_block,
                    source_info,
                    destination,
                    Constant { span: expr_span, user_ty: None, literal: this.hir.false_literal() },
                );

                // Link up both branches:
                this.cfg.goto(true_block, source_info, join_block);
                this.cfg.goto(false_block, source_info, join_block);
                join_block.unit()
            }
            ExprKind::Loop { body } => {
                // [block]
                //    |
                //   [loop_block] -> [body_block] -/eval. body/-> [body_block_end]
                //    |        ^                                         |
                // false link  |                                         |
                //    |        +-----------------------------------------+
                //    +-> [diverge_cleanup]
                // The false link is required to make sure borrowck considers unwinds through the
                // body, even when the exact code in the body cannot unwind

                let loop_block = this.cfg.start_new_block();

                // Start the loop.
                this.cfg.goto(block, source_info, loop_block);

                this.in_breakable_scope(
                    Some(loop_block),
                    destination,
                    scope,
                    expr_span,
                    move |this| {
                        // conduct the test, if necessary
                        let body_block = this.cfg.start_new_block();
                        this.cfg.terminate(
                            loop_block,
                            source_info,
                            TerminatorKind::FalseUnwind { real_target: body_block, unwind: None },
                        );
                        this.diverge_from(loop_block);

                        // The “return” value of the loop body must always be an unit. We therefore
                        // introduce a unit temporary as the destination for the loop body.
                        let tmp = this.get_unit_temp();
                        // Execute the body, branching back to the test.
                        // We don't need to provide a drop scope because `tmp`
                        // has type `()`.
                        let body_block_end = unpack!(this.into(tmp, None, body_block, body));
                        this.cfg.goto(body_block_end, source_info, loop_block);
                        schedule_drop(this);

                        // Loops are only exited by `break` expressions.
                        None
                    },
                )
            }
            ExprKind::Call { ty: _, fun, args, from_hir_call, fn_span } => {
                let fun = unpack!(block = this.as_local_operand(block, fun));
                let args: Vec<_> = args
                    .into_iter()
                    .map(|arg| unpack!(block = this.as_local_call_operand(block, arg)))
                    .collect();

                let success = this.cfg.start_new_block();

                this.record_operands_moved(&args);

                debug!("into_expr: fn_span={:?}", fn_span);

                this.cfg.terminate(
                    block,
                    source_info,
                    TerminatorKind::Call {
                        func: fun,
                        args,
                        cleanup: None,
                        // FIXME(varkor): replace this with an uninhabitedness-based check.
                        // This requires getting access to the current module to call
                        // `tcx.is_ty_uninhabited_from`, which is currently tricky to do.
                        destination: if expr.ty.is_never() {
                            None
                        } else {
                            Some((destination, success))
                        },
                        from_hir_call,
                        fn_span,
                    },
                );
                this.diverge_from(block);
                schedule_drop(this);
                success.unit()
            }
            ExprKind::Use { source } => this.into(destination, scope, block, source),
            ExprKind::Borrow { arg, borrow_kind } => {
                // We don't do this in `as_rvalue` because we use `as_place`
                // for borrow expressions, so we cannot create an `RValue` that
                // remains valid across user code. `as_rvalue` is usually called
                // by this method anyway, so this shouldn't cause too many
                // unnecessary temporaries.
                let arg_place = match borrow_kind {
                    BorrowKind::Shared => unpack!(block = this.as_read_only_place(block, arg)),
                    _ => unpack!(block = this.as_place(block, arg)),
                };
                let borrow =
                    Rvalue::Ref(this.hir.tcx().lifetimes.re_erased, borrow_kind, arg_place);
                this.cfg.push_assign(block, source_info, destination, borrow);
                block.unit()
            }
            ExprKind::AddressOf { mutability, arg } => {
                let place = match mutability {
                    hir::Mutability::Not => this.as_read_only_place(block, arg),
                    hir::Mutability::Mut => this.as_place(block, arg),
                };
                let address_of = Rvalue::AddressOf(mutability, unpack!(block = place));
                this.cfg.push_assign(block, source_info, destination, address_of);
                block.unit()
            }
            ExprKind::Adt { adt_def, variant_index, substs, user_ty, fields, base } => {
                // See the notes for `ExprKind::Array` in `as_rvalue` and for
                // `ExprKind::Borrow` above.
                let is_union = adt_def.is_union();
                let active_field_index = if is_union { Some(fields[0].name.index()) } else { None };

                let scope = this.local_scope();

                // first process the set of fields that were provided
                // (evaluating them in order given by user)
                let fields_map: FxHashMap<_, _> = fields
                    .into_iter()
                    .map(|f| (f.name, unpack!(block = this.as_operand(block, Some(scope), f.expr))))
                    .collect();

                let field_names = this.hir.all_fields(adt_def, variant_index);

                let fields: Vec<_> = if let Some(FruInfo { base, field_types }) = base {
                    let place_builder = unpack!(block = this.as_place_builder(block, base));

                    // MIR does not natively support FRU, so for each
                    // base-supplied field, generate an operand that
                    // reads it from the base.
                    field_names
                        .into_iter()
                        .zip(field_types.into_iter())
                        .map(|(n, ty)| match fields_map.get(&n) {
                            Some(v) => v.clone(),
                            None => {
                                let place_builder = place_builder.clone();
                                this.consume_by_copy_or_move(
                                    place_builder
                                        .field(n, ty)
                                        .into_place(this.hir.tcx(), this.hir.typeck_results()),
                                )
                            }
                        })
                        .collect()
                } else {
                    field_names.iter().filter_map(|n| fields_map.get(n).cloned()).collect()
                };

                let inferred_ty = expr.ty;
                let user_ty = user_ty.map(|ty| {
                    this.canonical_user_type_annotations.push(CanonicalUserTypeAnnotation {
                        span: source_info.span,
                        user_ty: ty,
                        inferred_ty,
                    })
                });
                let adt = box AggregateKind::Adt(
                    adt_def,
                    variant_index,
                    substs,
                    user_ty,
                    active_field_index,
                );
                this.record_operands_moved(&fields);
                this.cfg.push_assign(
                    block,
                    source_info,
                    destination,
                    Rvalue::Aggregate(adt, fields),
                );
                schedule_drop(this);
                block.unit()
            }
            ExprKind::InlineAsm { template, operands, options, line_spans } => {
                use crate::thir;
                use rustc_middle::mir;
                let operands = operands
                    .into_iter()
                    .map(|op| match op {
                        thir::InlineAsmOperand::In { reg, expr } => mir::InlineAsmOperand::In {
                            reg,
                            value: unpack!(block = this.as_local_operand(block, expr)),
                        },
                        thir::InlineAsmOperand::Out { reg, late, expr } => {
                            mir::InlineAsmOperand::Out {
                                reg,
                                late,
                                place: expr.map(|expr| unpack!(block = this.as_place(block, expr))),
                            }
                        }
                        thir::InlineAsmOperand::InOut { reg, late, expr } => {
                            let place = unpack!(block = this.as_place(block, expr));
                            mir::InlineAsmOperand::InOut {
                                reg,
                                late,
                                // This works because asm operands must be Copy
                                in_value: Operand::Copy(place),
                                out_place: Some(place),
                            }
                        }
                        thir::InlineAsmOperand::SplitInOut { reg, late, in_expr, out_expr } => {
                            mir::InlineAsmOperand::InOut {
                                reg,
                                late,
                                in_value: unpack!(block = this.as_local_operand(block, in_expr)),
                                out_place: out_expr.map(|out_expr| {
                                    unpack!(block = this.as_place(block, out_expr))
                                }),
                            }
                        }
                        thir::InlineAsmOperand::Const { expr } => mir::InlineAsmOperand::Const {
                            value: unpack!(block = this.as_local_operand(block, expr)),
                        },
                        thir::InlineAsmOperand::SymFn { expr } => {
                            mir::InlineAsmOperand::SymFn { value: box this.as_constant(expr) }
                        }
                        thir::InlineAsmOperand::SymStatic { def_id } => {
                            mir::InlineAsmOperand::SymStatic { def_id }
                        }
                    })
                    .collect();

                let destination = this.cfg.start_new_block();

                this.cfg.terminate(
                    block,
                    source_info,
                    TerminatorKind::InlineAsm {
                        template,
                        operands,
                        options,
                        line_spans,
                        destination: if options.contains(InlineAsmOptions::NORETURN) {
                            None
                        } else {
                            Some(destination)
                        },
                    },
                );
                destination.unit()
            }

            // These cases don't actually need a destination
            ExprKind::Assign { .. }
            | ExprKind::AssignOp { .. }
            | ExprKind::LlvmInlineAsm { .. } => {
                unpack!(block = this.stmt_expr(block, expr, None));
                this.cfg.push_assign_unit(block, source_info, destination, this.hir.tcx());
                block.unit()
            }

            ExprKind::Continue { .. } | ExprKind::Break { .. } | ExprKind::Return { .. } => {
                unpack!(block = this.stmt_expr(block, expr, None));
                // No assign, as these have type `!`.
                block.unit()
            }

            // Avoid creating a temporary
            ExprKind::VarRef { .. }
            | ExprKind::UpvarRef { .. }
            | ExprKind::PlaceTypeAscription { .. }
            | ExprKind::ValueTypeAscription { .. } => {
                debug_assert!(Category::of(&expr.kind) == Some(Category::Place));

                let place = unpack!(block = this.as_place(block, expr));
                let rvalue = Rvalue::Use(this.consume_by_copy_or_move(place));
                this.cfg.push_assign(block, source_info, destination, rvalue);
                schedule_drop(this);
                block.unit()
            }
            ExprKind::Index { .. } | ExprKind::Deref { .. } | ExprKind::Field { .. } => {
                debug_assert!(Category::of(&expr.kind) == Some(Category::Place));

                // Create a "fake" temporary variable so that we check that the
                // value is Sized. Usually, this is caught in type checking, but
                // in the case of box expr there is no such check.
                if !destination.projection.is_empty() {
                    this.local_decls.push(LocalDecl::new(expr.ty, expr.span));
                }

                debug_assert!(Category::of(&expr.kind) == Some(Category::Place));

                let place = unpack!(block = this.as_place(block, expr));
                let rvalue = Rvalue::Use(this.consume_by_copy_or_move(place));
                this.cfg.push_assign(block, source_info, destination, rvalue);
                schedule_drop(this);
                block.unit()
            }

            ExprKind::Yield { value } => {
                let scope = this.local_scope();
                let value = unpack!(block = this.as_operand(block, Some(scope), value));
                let resume = this.cfg.start_new_block();
                this.record_operands_moved(slice::from_ref(&value));
                this.cfg.terminate(
                    block,
                    source_info,
                    TerminatorKind::Yield { value, resume, resume_arg: destination, drop: None },
                );
                this.generator_drop_cleanup(block);
                schedule_drop(this);
                resume.unit()
            }

            // these are the cases that are more naturally handled by some other mode
            ExprKind::Unary { .. }
            | ExprKind::Binary { .. }
            | ExprKind::Box { .. }
            | ExprKind::Cast { .. }
            | ExprKind::Pointer { .. }
            | ExprKind::Repeat { .. }
            | ExprKind::Array { .. }
            | ExprKind::Tuple { .. }
            | ExprKind::Closure { .. }
            | ExprKind::ConstBlock { .. }
            | ExprKind::Literal { .. }
            | ExprKind::ThreadLocalRef(_)
            | ExprKind::StaticRef { .. } => {
                debug_assert!(match Category::of(&expr.kind).unwrap() {
                    // should be handled above
                    Category::Rvalue(RvalueFunc::Into) => false,

                    // must be handled above or else we get an
                    // infinite loop in the builder; see
                    // e.g., `ExprKind::VarRef` above
                    Category::Place => false,

                    _ => true,
                });

                let rvalue = unpack!(block = this.as_local_rvalue(block, expr));
                this.cfg.push_assign(block, source_info, destination, rvalue);
                schedule_drop(this);
                block.unit()
            }
        };

        if !expr_is_block_or_scope {
            let popped = this.block_context.pop();
            assert!(popped.is_some());
        }

        block_and
    }
}
