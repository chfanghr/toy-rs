use std::{mem, ops::Deref, rc::Rc};

use anyhow::{anyhow, Context, Ok, Result};
use chumsky::extra::Err;
use derive_getters::Getters;
use itertools::Either;

use crate::{
    parser::ast,
    utils::{
        assoc::Assoc,
        heap::{Addr, Heap},
        stack::Stack,
    },
};

#[derive(Debug, PartialEq, Eq, Clone)]
enum Instruction {
    Unwind,
    PushGlobal(ast::Name),
    PushNum(i64),
    Push(usize),
    MkAp,
    Slide(usize),
}

type Code = Vec<Instruction>;

#[derive(Debug, Clone)]
enum Node {
    Num(i64),
    Ap(Addr, Addr),
    Global(usize, Code),
}

#[derive(Debug, Clone, Getters)]
struct Stats {
    #[getter(copy)]
    steps: usize,
}

impl Stats {
    fn new() -> Self {
        Stats { steps: 0 }
    }

    fn incr_steps(&mut self) {
        self.steps += 1;
    }
}

#[derive(Debug, Clone, Getters)]
pub struct Machine {
    #[getter(skip)]
    code: Code,
    #[getter(skip)]
    instr_ptr: usize,
    #[getter(skip)]
    stack: Stack<Addr>,
    #[getter(skip)]
    heap: Heap<Node>,
    #[getter(skip)]
    globals: Assoc<ast::Name, Addr>,
    stats: Stats,
}

pub enum MachineHistoryIter {
    Machine(Machine),
    ErrOccurred(anyhow::Error),
    Done,
}

impl Iterator for MachineHistoryIter {
    type Item = Result<Machine>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            MachineHistoryIter::Machine(machine) => {
                let res = machine.clone();

                if let Err(err) = machine.step() {
                    *self = Self::ErrOccurred(err);
                } else if machine.is_done() {
                    *self = Self::Done
                }

                Some(Ok(res))
            }
            MachineHistoryIter::ErrOccurred(err) => {
                let err = mem::replace(err, anyhow::Error::msg("dummy"));
                let res = Some(Err(err));
                *self = Self::Done;
                res
            }
            MachineHistoryIter::Done => None,
        }
    }
}

impl MachineHistoryIter {
    fn new(machine: Machine) -> Self {
        Self::Machine(machine)
    }
}

impl Machine {
    pub(super) fn is_done(&self) -> bool {
        self.instr_ptr >= self.code.len()
    }

    pub(super) fn step(&mut self) -> Result<()> {
        let instr = self.code.get(self.instr_ptr).unwrap().clone();
        self.dispatch(instr)?;
        self.instr_ptr += 1;
        Ok(())
    }

    fn dispatch(&mut self, i: Instruction) -> Result<()> {
        match i {
            Instruction::Unwind => self.handle_unwind().context("Unwind"),
            Instruction::PushGlobal(name) => self.handle_push_global(name).context("PushGlobal"),
            Instruction::PushNum(i) => self.handle_push_num(i).context("PushNum"),
            Instruction::Push(n) => self.handle_push(n).context("Push"),
            Instruction::MkAp => self.handle_mk_ap().context("MkAp"),
            Instruction::Slide(n) => self.handle_slide(n).context("Slide"),
        }
    }

    fn handle_push_global(&mut self, name: ast::Name) -> Result<()> {
        let addr = self.lookup_global(&name)?;
        self.stack.push(addr);
        Ok(())
    }

    fn handle_push_num(&mut self, i: i64) -> Result<()> {
        let addr = self.heap.alloc(Node::Num(i));
        self.stack.push(addr);
        Ok(())
    }

    fn handle_mk_ap(&mut self) -> Result<()> {
        let l = self
            .stack
            .pop()
            .copied()
            .ok_or(anyhow!("l ptr not found"))?;
        let r = self
            .stack
            .pop()
            .copied()
            .ok_or(anyhow!("r ptr not found"))?;
        let node = Node::Ap(l, r);
        let addr = self.heap.alloc(node);
        self.stack.push(addr);
        Ok(())
    }

    fn handle_push(&mut self, n: usize) -> Result<()> {
        let n = n + 1;
        let addr = self
            .stack
            .peak_nth_from_top_cloned(1 + n)
            .ok_or(anyhow!("not enough elements on the stack: wanted {}", n))?;
        let r = self.must_extract_ap_node_r(addr);
        self.stack.push(r);
        Ok(())
    }

    fn handle_slide(&mut self, n: usize) -> Result<()> {
        let root = self
            .stack
            .pop()
            .copied()
            .expect("COMPILER BUG: root ptr missing");
        assert_eq!(
            self.stack.pop_n(n).len(),
            n,
            "COMPILER BUG: not enough args, should have been caught when executing Push"
        );
        self.stack.push(root);
        Ok(())
    }

    fn handle_unwind(&mut self) -> Result<()> {
        let addr = self
            .stack
            .peak()
            .copied()
            .expect("COMPILER BUG: trying to unwind an empty stack");

        match self.must_access_node(addr) {
            Node::Num(_) => (),
            Node::Ap(l, _) => {
                self.stack.push(*l);
                self.instr_ptr -= 1;
            }
            Node::Global(_, instrs) => {
                self.code = instrs.clone();
                self.instr_ptr = 0;
            }
        };
        Ok(())
    }

    fn must_extract_ap_node_r(&self, addr: Addr) -> Addr {
        let node = self.must_access_node(addr);
        let r = if let Node::Ap(_, r) = node {
            *r
        } else {
            panic!(
                "COMPILER BUG: expects an Ap node at {:?}, got {:?}",
                addr, node
            )
        };
        r
    }

    fn must_access_node(&self, addr: Addr) -> &Node {
        self.heap.access(addr).unwrap()
    }

    fn lookup_global(&self, name: &ast::Name) -> Result<Addr> {
        self.globals
            .lookup(name)
            .map(|x| *x)
            .ok_or(anyhow!("global not found: {:?}", name))
    }

    pub fn history(self) -> MachineHistoryIter {
        MachineHistoryIter::new(self)
    }
}

pub mod postfix_eval {
    use std::collections::LinkedList;

    use chumsky::container::Container;

    #[derive(Debug, Clone)]
    pub enum Expr {
        Num(i64),
        Plus(Box<Expr>, Box<Expr>),
        Mul(Box<Expr>, Box<Expr>),
    }

    pub fn tree_eval(expr: Expr) -> i64 {
        match expr {
            Expr::Num(i) => i,
            Expr::Plus(l, r) => tree_eval(*l) + tree_eval(*r),
            Expr::Mul(l, r) => tree_eval(*l) * tree_eval(*r),
        }
    }

    #[derive(Debug)]
    pub enum Instruction {
        Num(i64),
        Plus,
        Mul,
    }

    #[derive(Debug)]
    pub struct Machine {
        instructions: LinkedList<Instruction>,
        stack: LinkedList<i64>,
    }

    impl Machine {
        pub fn new(instrs: Vec<Instruction>) -> Self {
            Self {
                instructions: instrs.into_iter().collect(),
                stack: LinkedList::new(),
            }
        }

        pub fn execute(mut self) -> i64 {
            self.run();
            self.stack.pop_back().unwrap()
        }

        fn step(&mut self, i: Instruction) {
            match i {
                Instruction::Num(i) => self.stack.push_front(i),
                Instruction::Plus => {
                    let l = self.stack.pop_front().unwrap();
                    let r = self.stack.pop_front().unwrap();
                    self.stack.push_front(l + r)
                }
                Instruction::Mul => {
                    let l = self.stack.pop_front().unwrap();
                    let r = self.stack.pop_front().unwrap();
                    self.stack.push_front(l * r)
                }
            }
        }

        fn run(&mut self) {
            while let Some(instr) = self.instructions.pop_front() {
                self.step(instr);
            }
        }
    }

    #[derive(Debug)]
    enum Imm {
        ToCompile(Expr),
        Compiled(Instruction),
    }

    pub fn compile(expr: Expr) -> Vec<Instruction> {
        let mut imm_stack = LinkedList::<Imm>::new();
        let mut instrs = Vec::new();
        imm_stack.push(Imm::ToCompile(expr));
        while let Some(imm) = imm_stack.pop_front() {
            match imm {
                Imm::ToCompile(expr) => match expr {
                    Expr::Num(i) => imm_stack.push_front(Imm::Compiled(Instruction::Num(i))),
                    Expr::Plus(l, r) => {
                        imm_stack.push_front(Imm::Compiled(Instruction::Plus));
                        imm_stack.push_front(Imm::ToCompile(*r));
                        imm_stack.push_front(Imm::ToCompile(*l));
                    }
                    Expr::Mul(l, r) => {
                        imm_stack.push_front(Imm::Compiled(Instruction::Mul));
                        imm_stack.push_front(Imm::ToCompile(*r));
                        imm_stack.push_front(Imm::ToCompile(*l));
                    }
                },
                Imm::Compiled(i) => instrs.push(i),
            }
        }
        instrs
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn test() {
            let expr = Expr::Plus(
                Box::new(Expr::Num(2)),
                Box::new(Expr::Mul(Box::new(Expr::Num(3)), Box::new(Expr::Num(4)))),
            );
            let postfix_eval_result = Machine::new(compile(expr.clone())).execute();
            let tree_eval_result = tree_eval(expr);
            assert_eq!(postfix_eval_result, tree_eval_result)
        }
    }
}
