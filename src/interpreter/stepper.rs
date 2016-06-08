use super::{
    FnEvalContext,
    CachedMir,
    TerminatorTarget,
    ConstantId,
    GlobalEvalContext,
};
use error::EvalResult;
use rustc::mir::repr as mir;
use rustc::ty::subst::{self, Subst};
use rustc::hir::def_id::DefId;
use rustc::mir::visit::{Visitor, LvalueContext};
use syntax::codemap::Span;
use std::rc::Rc;
use memory::Pointer;

pub enum Event {
    Constant,
    Assignment,
    Terminator,
    Done,
}

pub struct Stepper<'fncx, 'a: 'fncx, 'b: 'a + 'mir, 'mir: 'fncx, 'tcx: 'b>{
    fncx: &'fncx mut FnEvalContext<'a, 'b, 'mir, 'tcx>,
    process: fn (&mut Stepper<'fncx, 'a, 'b, 'mir, 'tcx>) -> EvalResult<()>,

    // a cache of the constants to be computed before the next statement/terminator
    // this is an optimization, so we don't have to allocate a new vector for every statement
    constants: Vec<(ConstantId<'tcx>, Span, Pointer, CachedMir<'mir, 'tcx>)>,
}

impl<'fncx, 'a, 'b: 'a + 'mir, 'mir, 'tcx: 'b> Stepper<'fncx, 'a, 'b, 'mir, 'tcx> {
    pub(super) fn new(fncx: &'fncx mut FnEvalContext<'a, 'b, 'mir, 'tcx>) -> Self {
        Stepper {
            fncx: fncx,
            process: Self::dummy,
            constants: Vec::new(),
        }
    }

    fn dummy(&mut self) -> EvalResult<()> { Ok(()) }

    fn statement(&mut self) -> EvalResult<()> {
        let mir = self.fncx.mir();
        let block_data = mir.basic_block_data(self.fncx.frame().next_block);
        let stmt = &block_data.statements[self.fncx.frame().stmt];
        let mir::StatementKind::Assign(ref lvalue, ref rvalue) = stmt.kind;
        let result = self.fncx.eval_assignment(lvalue, rvalue);
        self.fncx.maybe_report(result)?;
        self.fncx.frame_mut().stmt += 1;
        Ok(())
    }

    fn terminator(&mut self) -> EvalResult<()> {
        // after a terminator we go to a new block
        self.fncx.frame_mut().stmt = 0;
        let term = {
            let mir = self.fncx.mir();
            let block_data = mir.basic_block_data(self.fncx.frame().next_block);
            let terminator = block_data.terminator();
            let result = self.fncx.eval_terminator(terminator);
            self.fncx.maybe_report(result)?
        };
        match term {
            TerminatorTarget::Block => {},
            TerminatorTarget::Return => {
                self.fncx.pop_stack_frame();
            },
            TerminatorTarget::Call => {},
        }
        Ok(())
    }

    pub fn step(&mut self) -> EvalResult<Event> {
        (self.process)(self)?;

        if self.fncx.stack.is_empty() {
            // fuse the iterator
            self.process = Self::dummy;
            return Ok(Event::Done);
        }

        let block = self.fncx.frame().next_block;
        let stmt = self.fncx.frame().stmt;
        let mir = self.fncx.mir();
        let basic_block = mir.basic_block_data(block);

        if let Some(ref stmt) = basic_block.statements.get(stmt) {
            assert!(self.constants.is_empty());
            ConstantExtractor {
                span: stmt.span,
                substs: self.fncx.substs(),
                def_id: self.fncx.frame().def_id,
                gecx: self.fncx.gecx,
                constants: &mut self.constants,
                mir: &mir,
            }.visit_statement(block, stmt);
            if self.constants.is_empty() {
                self.process = Self::statement;
                return Ok(Event::Assignment);
            } else {
                self.process = Self::statement;
                self.extract_constants();
                return Ok(Event::Constant);
            }
        }

        let terminator = basic_block.terminator();
        assert!(self.constants.is_empty());
        ConstantExtractor {
            span: terminator.span,
            substs: self.fncx.substs(),
            def_id: self.fncx.frame().def_id,
            gecx: self.fncx.gecx,
            constants: &mut self.constants,
            mir: &mir,
        }.visit_terminator(block, terminator);
        if self.constants.is_empty() {
            self.process = Self::terminator;
            Ok(Event::Terminator)
        } else {
            self.process = Self::statement;
            self.extract_constants();
            Ok(Event::Constant)
        }
    }

    fn extract_constants(&mut self) {
        assert!(!self.constants.is_empty());
        for (cid, span, return_ptr, mir) in self.constants.drain(..) {
            let def_id = cid.def_id();
            let substs = cid.substs();
            self.fncx.push_stack_frame(def_id, span, mir, substs, Some(return_ptr));
        }
    }

    /// returns the statement that will be processed next
    pub fn stmt(&self) -> &mir::Statement {
        &self.fncx.basic_block().statements[self.fncx.frame().stmt]
    }

    /// returns the terminator of the current block
    pub fn term(&self) -> &mir::Terminator {
        self.fncx.basic_block().terminator()
    }

    pub fn block(&self) -> mir::BasicBlock {
        self.fncx.frame().next_block
    }
}

struct ConstantExtractor<'a, 'b: 'mir, 'mir: 'a, 'tcx: 'b> {
    span: Span,
    constants: &'a mut Vec<(ConstantId<'tcx>, Span, Pointer, CachedMir<'mir, 'tcx>)>,
    gecx: &'a mut GlobalEvalContext<'b, 'tcx>,
    mir: &'a mir::Mir<'tcx>,
    def_id: DefId,
    substs: &'tcx subst::Substs<'tcx>,
}

impl<'a, 'b, 'mir, 'tcx> ConstantExtractor<'a, 'b, 'mir, 'tcx> {
    fn static_item(&mut self, def_id: DefId, substs: &'tcx subst::Substs<'tcx>, span: Span) {
        let cid = ConstantId::Static {
            def_id: def_id,
            substs: substs,
        };
        if self.gecx.statics.contains_key(&cid) {
            return;
        }
        let mir = self.gecx.load_mir(def_id);
        let ptr = self.gecx.alloc_ret_ptr(mir.return_ty, substs).expect("there's no such thing as an unreachable static");
        self.gecx.statics.insert(cid.clone(), ptr);
        self.constants.push((cid, span, ptr, mir));
    }
}

impl<'a, 'b, 'mir, 'tcx> Visitor<'tcx> for ConstantExtractor<'a, 'b, 'mir, 'tcx> {
    fn visit_constant(&mut self, constant: &mir::Constant<'tcx>) {
        self.super_constant(constant);
        match constant.literal {
            // already computed by rustc
            mir::Literal::Value { .. } => {}
            mir::Literal::Item { def_id, substs } => {
                let item_ty = self.gecx.tcx.lookup_item_type(def_id).subst(self.gecx.tcx, substs);
                if item_ty.ty.is_fn() {
                    // unimplemented
                } else {
                    self.static_item(def_id, substs, constant.span);
                }
            },
            mir::Literal::Promoted { index } => {
                let cid = ConstantId::Promoted {
                    def_id: self.def_id,
                    substs: self.substs,
                    index: index,
                };
                if self.gecx.statics.contains_key(&cid) {
                    return;
                }
                let mir = self.mir.promoted[index].clone();
                let return_ty = mir.return_ty;
                let return_ptr = self.gecx.alloc_ret_ptr(return_ty, cid.substs()).expect("there's no such thing as an unreachable static");
                let mir = CachedMir::Owned(Rc::new(mir));
                self.gecx.statics.insert(cid.clone(), return_ptr);
                self.constants.push((cid, constant.span, return_ptr, mir));
            }
        }
    }

    fn visit_lvalue(&mut self, lvalue: &mir::Lvalue<'tcx>, context: LvalueContext) {
        self.super_lvalue(lvalue, context);
        if let mir::Lvalue::Static(def_id) = *lvalue {
            let substs = self.gecx.tcx.mk_substs(subst::Substs::empty());
            let span = self.span;
            self.static_item(def_id, substs, span);
        }
    }
}
