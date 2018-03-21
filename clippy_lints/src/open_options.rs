use rustc::hir::{Expr, ExprLit, ExprMethodCall};
use rustc::lint::*;
use syntax::ast::LitKind;
use syntax::codemap::{Span, Spanned};
use utils::{match_type, paths, span_lint, walk_ptrs_ty};

/// **What it does:** Checks for duplicate open options as well as combinations
/// that make no sense.
///
/// **Why is this bad?** In the best case, the code will be harder to read than
/// necessary. I don't know the worst case.
///
/// **Known problems:** None.
///
/// **Example:**
/// ```rust
/// OpenOptions::new().read(true).truncate(true)
/// ```
declare_lint! {
    pub NONSENSICAL_OPEN_OPTIONS,
    Warn,
    "nonsensical combination of options for opening a file"
}

#[derive(Copy, Clone)]
pub struct NonSensical;

impl LintPass for NonSensical {
    fn get_lints(&self) -> LintArray {
        lint_array!(NONSENSICAL_OPEN_OPTIONS)
    }
}

impl<'a, 'tcx> LateLintPass<'a, 'tcx> for NonSensical {
    fn check_expr(&mut self, cx: &LateContext<'a, 'tcx>, e: &'tcx Expr) {
        if let ExprMethodCall(ref path, _, ref arguments) = e.node {
            let obj_ty = walk_ptrs_ty(cx.tables.expr_ty(&arguments[0]));
            if path.name == "open" && match_type(cx, obj_ty, &paths::OPEN_OPTIONS) {
                let mut options = Vec::new();
                get_open_options(cx, &arguments[0], &mut options);
                check_open_options(cx, &options, e.span);
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Argument {
    True,
    False,
    Unknown,
}

#[derive(Debug)]
enum OpenOption {
    Write,
    Read,
    Truncate,
    Create,
    Append,
}

fn get_open_options(cx: &LateContext, argument: &Expr, options: &mut Vec<(OpenOption, Argument)>) {
    if let ExprMethodCall(ref path, _, ref arguments) = argument.node {
        let obj_ty = walk_ptrs_ty(cx.tables.expr_ty(&arguments[0]));

        // Only proceed if this is a call on some object of type std::fs::OpenOptions
        if match_type(cx, obj_ty, &paths::OPEN_OPTIONS) && arguments.len() >= 2 {
            let argument_option = match arguments[1].node {
                ExprLit(ref span) => {
                    if let Spanned {
                        node: LitKind::Bool(lit),
                        ..
                    } = **span
                    {
                        if lit {
                            Argument::True
                        } else {
                            Argument::False
                        }
                    } else {
                        return; // The function is called with a literal
                                // which is not a boolean literal. This is theoretically
                                // possible, but not very likely.
                    }
                },
                _ => Argument::Unknown,
            };

            match &*path.name.as_str() {
                "create" => {
                    options.push((OpenOption::Create, argument_option));
                },
                "append" => {
                    options.push((OpenOption::Append, argument_option));
                },
                "truncate" => {
                    options.push((OpenOption::Truncate, argument_option));
                },
                "read" => {
                    options.push((OpenOption::Read, argument_option));
                },
                "write" => {
                    options.push((OpenOption::Write, argument_option));
                },
                _ => (),
            }

            get_open_options(cx, &arguments[0], options);
        }
    }
}

fn check_open_options(cx: &LateContext, options: &[(OpenOption, Argument)], span: Span) {
    let (mut create, mut append, mut truncate, mut read, mut write) = (false, false, false, false, false);
    let (mut create_arg, mut append_arg, mut truncate_arg, mut read_arg, mut write_arg) =
        (false, false, false, false, false);
    // This code is almost duplicated (oh, the irony), but I haven't found a way to
    // unify it.

    for option in options {
        match *option {
            (OpenOption::Create, arg) => {
                if create {
                    span_lint(
                        cx,
                        NONSENSICAL_OPEN_OPTIONS,
                        span,
                        "the method \"create\" is called more than once",
                    );
                } else {
                    create = true
                }
                create_arg = create_arg || (arg == Argument::True);;
            },
            (OpenOption::Append, arg) => {
                if append {
                    span_lint(
                        cx,
                        NONSENSICAL_OPEN_OPTIONS,
                        span,
                        "the method \"append\" is called more than once",
                    );
                } else {
                    append = true
                }
                append_arg = append_arg || (arg == Argument::True);;
            },
            (OpenOption::Truncate, arg) => {
                if truncate {
                    span_lint(
                        cx,
                        NONSENSICAL_OPEN_OPTIONS,
                        span,
                        "the method \"truncate\" is called more than once",
                    );
                } else {
                    truncate = true
                }
                truncate_arg = truncate_arg || (arg == Argument::True);
            },
            (OpenOption::Read, arg) => {
                if read {
                    span_lint(
                        cx,
                        NONSENSICAL_OPEN_OPTIONS,
                        span,
                        "the method \"read\" is called more than once",
                    );
                } else {
                    read = true
                }
                read_arg = read_arg || (arg == Argument::True);;
            },
            (OpenOption::Write, arg) => {
                if write {
                    span_lint(
                        cx,
                        NONSENSICAL_OPEN_OPTIONS,
                        span,
                        "the method \"write\" is called more than once",
                    );
                } else {
                    write = true
                }
                write_arg = write_arg || (arg == Argument::True);;
            },
        }
    }

    if read && truncate && read_arg && truncate_arg && !(write && write_arg) {
        span_lint(cx, NONSENSICAL_OPEN_OPTIONS, span, "file opened with \"truncate\" and \"read\"");
    }
    if append && truncate && append_arg && truncate_arg {
        span_lint(
            cx,
            NONSENSICAL_OPEN_OPTIONS,
            span,
            "file opened with \"append\" and \"truncate\"",
        );
    }
}
