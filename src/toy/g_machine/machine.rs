use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, LinkedList},
    mem,
    ops::Deref,
    rc::Rc,
};

use anyhow::{Context, Ok, Result, anyhow, bail};
use intmap::IntMap;
use log::debug;
use stacksafe::StackSafe;

use crate::{
    g_machine::types::*,
    parser::ast,
    utils::{
        assoc::Assoc,
        heap_v2::{Addr, Heap},
        stack::Stack,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Num(i64),
    Ap(Addr, Addr),
    Global(usize, Code),
    Indirect(Addr),
}

impl Node {
    fn extract_ap(&self) -> Option<(Addr, Addr)> {
        match self {
            Self::Ap(l, r) => Some((*l, *r)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Stats {
    steps: usize,
}

impl Stats {
    pub fn steps(&self) -> usize {
        self.steps
    }
}

#[derive(Debug, Clone)]
struct ExecFrame {
    code_stack: Vec<(Code, usize)>,
    stack: Stack<Addr>,
}

impl ExecFrame {
    fn empty() -> Self {
        Self {
            code_stack: vec![],
            stack: Stack::new(),
        }
    }

    fn new(code: Code) -> Self {
        let code_stack = if code.0.len() == 0 {
            vec![]
        } else {
            vec![(code, 0usize)]
        };

        Self {
            code_stack,
            stack: Stack::new(),
        }
    }

    fn new_eval_at(addr: Addr) -> Self {
        let code = Code::new(vec![Instruction::Unwind]);

        let mut stack = Stack::new();
        stack.push(addr);

        Self {
            code_stack: vec![(code, 0)],
            stack,
        }
    }

    fn current_instr(&self) -> Option<Instruction> {
        let (code, idx) = self.code_stack.last()?;
        code.0.get(*idx).cloned()
    }

    fn advance_instr_ptr(&mut self) {
        while let Some((code, idx)) = self.code_stack.last_mut() {
            let idx_next = *idx + 1;
            if code.0.get(idx_next).is_some() {
                *idx = idx_next;
                break;
            } else {
                self.code_stack.pop();
            }
        }
    }

    fn checked_pop_n(&mut self, n: usize) -> Result<Vec<Addr>> {
        let res = self.stack.pop_n_cloned(n);
        let n_popped = res.len();
        if n_popped != n {
            bail!(
                "expected at least {} operands on the stack, got {}",
                n,
                n_popped
            )
        }
        Ok(res)
    }

    fn push_code(&mut self, code: Code) {
        self.code_stack.push((code, 0));
    }

    fn is_last_instruction(&self) -> bool {
        self.code_stack.len() == 1 && {
            let (code, instr_ptr) = self.code_stack.first().unwrap();
            code.0.len() - 1 == *instr_ptr
        }
    }
}

#[derive(Debug, Clone)]
struct Machine {
    current: ExecFrame,
    dump: Vec<ExecFrame>,
    heap: Heap<Node>,
    globals: HashMap<ast::Name, Addr>,
    integers: IntMap<i64, Addr>,
}

impl Machine {
    fn new(p: CompiledProgram) -> Self {
        let current = ExecFrame::new(Code::new(vec![
            Instruction::PushGlobal(ast::Name::new("main")),
            Instruction::Eval,
        ]));

        let (heap, globals) = Self::build_initial_heap(p);

        let this = Self {
            current,
            dump: vec![],
            heap,
            globals,
            integers: IntMap::new(),
        };

        this
    }

    fn build_initial_heap(p: CompiledProgram) -> (Heap<Node>, HashMap<ast::Name, Addr>) {
        let mut heap = Heap::new();
        let mut globals = HashMap::new();

        for (name, (n_args, code)) in p.0.into_iter() {
            let addr = heap.alloc(Node::Global(n_args, code));
            globals.insert(name, addr);
        }

        (heap, globals)
    }

    fn run(&mut self) -> Result<()> {
        while let advanced = self.step()?
            && advanced
        {
            self.do_admin()?;
        }

        Ok(())
    }

    fn step(&mut self) -> Result<bool> {
        let advanced = if let Some(instr) = self.current.current_instr() {
            self.dispatch(instr)?;
            true
        } else {
            self.restore_context()
        };

        Ok(advanced)
    }

    fn do_admin(&mut self) -> Result<()> {
        Ok(())
    }

    fn dispatch(&mut self, i: Instruction) -> Result<()> {
        match i {
            Instruction::Unwind => self.handle_unwind().context("Unwind"),
            Instruction::PushGlobal(n) => self.handle_push_global(n).context("PushGlobal"),
            Instruction::PushNum(n) => self.handle_push_num(n).context("PushNum"),
            Instruction::Push(n) => self.handle_push(n).context("Push"),
            Instruction::MkAp => self.handle_mk_ap().context("MkAp"),
            Instruction::Update(n) => self.handle_update(n).context("Update"),
            Instruction::Pop(n) => self.handle_pop(n).context("Pop"),
            Instruction::Alloc(n) => self.handle_alloc(n).context("Alloc"),
            Instruction::Slide(n) => self.handle_slide(n).context("Slide"),
            Instruction::Eval => self.handle_eval().context("Eval"),
            Instruction::Add => self.handle_add().context("Add"),
            Instruction::Sub => todo!(),
            Instruction::Mul => todo!(),
            Instruction::Div => todo!(),
            Instruction::Eq => todo!(),
            Instruction::Ne => todo!(),
            Instruction::Gt => todo!(),
            Instruction::Ge => todo!(),
            Instruction::Lt => todo!(),
            Instruction::Le => todo!(),
            Instruction::BooleanAnd => todo!(),
            Instruction::BooleanOr => todo!(),
            Instruction::Branch(then_branch, else_branch) => self
                .handle_branch(&then_branch, &else_branch)
                .context("Branch"),
        }
    }

    // Instructions

    fn handle_unwind(&mut self) -> Result<()> {
        if !self.current.is_last_instruction() {
            bail!("unwind should be the last instruction to be executed")
        };

        let addr = self
            .current
            .stack
            .peak_cloned()
            .ok_or(anyhow!("cannot unwind a empty stack"))?;

        match self.must_access_node(addr) {
            Node::Num(i) => self.unwind_num(addr, *i),
            Node::Ap(l, r) => self.unwind_ap(*l, *r),
            Node::Global(n_args, code) => self.unwind_global(*n_args, code.clone()),
            Node::Indirect(addr) => self.unwind_indirect(*addr),
        }
    }

    fn unwind_num(&mut self, addr: Addr, _i: i64) -> Result<()> {
        if self.restore_context() {
            self.current.stack.push(addr);
        } else {
            self.current.advance_instr_ptr();
        }

        Ok(())
    }

    fn unwind_ap(&mut self, l: Addr, r: Addr) -> Result<()> {
        self.current.stack.push(l);
        Ok(())
    }

    fn unwind_global(&mut self, n_args: usize, code: Code) -> Result<()> {
        _ = self.current.stack.pop(); // the global node itself

        if n_args > 0 {
            let ap_addresses = self.current.stack.pop_n_cloned(n_args);

            let last_ap_node_addr = *ap_addresses.last().unwrap();

            let args_addresses = ap_addresses
                .iter()
                .map(|a| {
                    self.must_access_node(self.follow_indirect(*a))
                        .extract_ap()
                        .map(|p| p.1)
                })
                .collect::<Option<Vec<_>>>()
                .ok_or(anyhow!(
                    "expected all ap nodes in addresses: {:?}",
                    ap_addresses
                ))?;

            if args_addresses.len() == n_args {
                self.current.stack.push(last_ap_node_addr); // to be overridden

                for addr in args_addresses.into_iter().rev() {
                    self.current.stack.push(addr);
                }

                self.current.advance_instr_ptr();
                self.current.push_code(code);
            } else {
                debug!("handle unsaturated super combinator call");

                if self.restore_context() {
                    self.current.stack.push(last_ap_node_addr);
                } else {
                    self.current.advance_instr_ptr();
                }
            };
        }
        Ok(())
    }

    fn unwind_indirect(&mut self, addr: Addr) -> Result<()> {
        self.current.stack.pop_cloned().unwrap();
        self.current.stack.push(addr);
        Ok(())
    }

    fn handle_push_global(&mut self, name: ast::Name) -> Result<()> {
        let addr = self.lookup_global(&name)?;
        self.current.stack.push(addr);

        self.current.advance_instr_ptr();

        Ok(())
    }

    fn handle_push_num(&mut self, i: i64) -> Result<()> {
        let addr = self.alloc_num_node(i);
        self.current.stack.push(addr);

        self.current.advance_instr_ptr();

        Ok(())
    }

    fn handle_push(&mut self, n: usize) -> Result<()> {
        let addr = self
            .current
            .stack
            .peak_nth_from_top_cloned(n)
            .ok_or(anyhow!(
                "expect at least {} operands on the stack, while stack height is {}",
                n + 1,
                self.current.stack.height()
            ))?;
        self.current.stack.push(addr);

        self.current.advance_instr_ptr();

        Ok(())
    }

    fn handle_mk_ap(&mut self) -> Result<()> {
        let l = self
            .current
            .stack
            .pop()
            .copied()
            .ok_or(anyhow!("lhs ptr not found"))?;
        let r = self
            .current
            .stack
            .pop()
            .copied()
            .ok_or(anyhow!("rhs ptr not found"))?;
        let node = Node::Ap(l, r);
        let addr = self.heap.alloc(node);

        self.current.stack.push(addr);

        self.current.advance_instr_ptr();

        Ok(())
    }

    fn handle_update(&mut self, n: usize) -> Result<()> {
        let src_addr = self
            .current
            .stack
            .pop_cloned()
            .ok_or(anyhow!("src addr not found"))?;
        let dest_addr = self
            .current
            .stack
            .peak_nth_from_top_cloned(n)
            .ok_or(anyhow!("dest addr not found"))?;

        if src_addr == dest_addr {
            bail!(
                "infinite loop: src addr equals to dest addr: {:?}",
                src_addr
            );
        }

        let node = Node::Indirect(src_addr);

        self.replace_node_at(src_addr, node);

        self.current.advance_instr_ptr();

        Ok(())
    }

    fn handle_pop(&mut self, n: usize) -> Result<()> {
        self.current.checked_pop_n(n)?;

        self.current.advance_instr_ptr();

        Ok(())
    }

    fn handle_alloc(&mut self, n: usize) -> Result<()> {
        for _ in 0..n {
            let addr = self.heap.alloc(Node::Indirect(Addr::null()));
            self.current.stack.push(addr);
        }

        self.current.advance_instr_ptr();

        Ok(())
    }

    fn handle_slide(&mut self, n: usize) -> Result<()> {
        let addr = self
            .current
            .stack
            .pop_cloned()
            .ok_or(anyhow!("slide on empty stack"))?;

        self.current.checked_pop_n(n)?;

        self.current.stack.push(addr);

        self.current.advance_instr_ptr();

        Ok(())
    }

    fn handle_eval(&mut self) -> Result<()> {
        let addr = self
            .current
            .stack
            .pop_cloned()
            .ok_or(anyhow!("nothing to evaluate"))?;

        self.save_replace_context(ExecFrame::new_eval_at(addr));

        Ok(())
    }

    fn handle_add(&mut self) -> Result<()> {
        self.impl_binary_numerical_prim_op(|l, r| Ok(l + r))
    }

    fn handle_branch(
        &mut self,
        then_branch: &StackSafe<Code>,
        else_branch: &StackSafe<Code>,
    ) -> Result<()> {
        let pred_addr = self
            .current
            .stack
            .pop_cloned()
            .ok_or(anyhow!("predicate address not found"))?;
        let pred_node = self.must_access_node(pred_addr);

        // FIXME: Use Constr once we have it
        let pred = match pred_node {
            Node::Num(i) => unbox_boolean(*i),
            _ => None,
        }
        .ok_or(anyhow!("unrecognized boolean: node {:?}", pred_node))?;

        self.current.advance_instr_ptr();

        let next = (if pred { then_branch } else { else_branch })
            .deref()
            .clone();

        self.current.push_code(next);

        Ok(())
    }

    // Helpers

    fn alloc_num_node(&mut self, i: i64) -> Addr {
        match self.integers.get(i) {
            Some(addr) => *addr,
            None => {
                let addr = self.heap.alloc(Node::Num(i));
                self.integers.insert(i, addr);
                addr
            }
        }
    }

    fn lookup_global(&self, name: &ast::Name) -> Result<Addr> {
        self.globals
            .get(name)
            .map(|x| *x)
            .ok_or(anyhow!("global not found: {:?}", name))
    }

    fn must_access_node(&self, addr: Addr) -> &Node {
        self.heap
            .access(addr)
            .expect(&format!("cannot access node at {:?}", addr))
    }

    fn must_access_node_mut(&mut self, addr: Addr) -> &mut Node {
        self.heap
            .access_mut(addr)
            .expect(&format!("cannot access node at {:?}", addr))
    }

    fn replace_node_at(&mut self, a: Addr, node: Node) {
        _ = mem::replace(self.must_access_node_mut(a), node);
    }

    fn save_replace_context(&mut self, new_context: ExecFrame) {
        let old_context = mem::replace(&mut self.current, new_context);
        self.dump.push(old_context);
    }

    fn restore_context(&mut self) -> bool {
        if let Some(frame) = self.dump.pop() {
            self.current = frame;
            true
        } else {
            false
        }
    }

    fn follow_indirect(&self, addr: Addr) -> Addr {
        let mut addr = addr;

        loop {
            match self.must_access_node(addr) {
                Node::Indirect(addr_next) => addr = *addr_next,
                _ => break,
            }
        }

        addr
    }

    fn impl_binary_numerical_prim_op<F>(&mut self, f: F) -> Result<()>
    where
        F: Fn(i64, i64) -> Result<i64>,
    {
        let lhs_addr = *self.current.stack.pop().ok_or(anyhow!("lhs not found"))?;
        let rhs_addr = *self.current.stack.pop().ok_or(anyhow!("rhs not found"))?;

        let lhs = self.must_access_node(lhs_addr);
        let rhs = self.must_access_node(rhs_addr);

        let res = match (lhs, rhs) {
            (Node::Num(l), Node::Num(r)) => f(*l, *r),
            _ => Err(anyhow!(
                "expected lhs and rhs to be in WHNF and both numbers, got: lhs: {:?}, rhs {:?}",
                lhs,
                rhs
            )),
        }?;

        let addr = self.alloc_num_node(res);

        self.current.stack.push(addr);

        Ok(())
    }
}

fn unbox_boolean(i: i64) -> Option<bool> {
    match i {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}
