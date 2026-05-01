use std::{collections::LinkedList, iter, mem, rc::Rc};

use anyhow::{anyhow, Context, Ok, Result};
use derive_getters::Getters;

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
    Global(usize, Rc<Code>),
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
    code: Rc<Code>,
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
        let code = self.code.clone();
        let instr = code.get(self.instr_ptr).unwrap();
        self.dispatch(instr)?;
        self.instr_ptr += 1;
        Ok(())
    }

    fn dispatch(&mut self, i: &Instruction) -> Result<()> {
        match i {
            Instruction::Unwind => self.handle_unwind().context("Unwind"),
            Instruction::PushGlobal(name) => self.handle_push_global(name).context("PushGlobal"),
            Instruction::PushNum(i) => self.handle_push_num(*i).context("PushNum"),
            Instruction::Push(n) => self.handle_push(*n).context("Push"),
            Instruction::MkAp => self.handle_mk_ap().context("MkAp"),
            Instruction::Slide(n) => self.handle_slide(*n).context("Slide"),
        }
    }

    fn handle_push_global(&mut self, name: &ast::Name) -> Result<()> {
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
                self.instr_ptr -= 1; // To unwind again
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompiledSuperCombinator {
    name: ast::Name,
    n_args: usize,
    code: Code,
}

fn compile_sc(sc: ast::SuperCombinator<ast::Name>) -> Result<CompiledSuperCombinator> {
    let env: Assoc<ast::Name, usize> = sc
        .arguments
        .into_iter()
        .enumerate()
        .map(|(x, y)| (y, x))
        .collect();
    let n_env = env.size();
    let code = compile_expr(sc.body, env)?;
    Ok(CompiledSuperCombinator {
        name: sc.name,
        n_args: n_env,
        code,
    })
}

enum CompilationTodo {
    ToCompile(ast::Expr<ast::Name>, usize),
    Done(Instruction),
}

fn compile_expr(e: ast::Expr<ast::Name>, env: Assoc<ast::Name, usize>) -> Result<Code> {
    let mut todo_stack = LinkedList::<CompilationTodo>::new();
    let mut code = Code::new();

    todo_stack.push_front(CompilationTodo::ToCompile(e, 0));

    while let Some(todo) = todo_stack.pop_front() {
        match todo {
            CompilationTodo::ToCompile(expr, added_offset) => match expr {
                ast::Expr::Var(name) => todo_stack.push_front(CompilationTodo::Done(
                    env.lookup(&name)
                        .map_or(Instruction::PushGlobal(name), |offset| {
                            Instruction::Push(*offset + added_offset)
                        }),
                )),
                ast::Expr::Num(i) => {
                    todo_stack.push_front(CompilationTodo::Done(Instruction::PushNum(i.0)))
                }
                ast::Expr::Ap(ap) => {
                    let ap = *ap;
                    todo_stack.push_front(CompilationTodo::Done(Instruction::MkAp));
                    todo_stack.push_front(CompilationTodo::ToCompile(ap.l, added_offset + 1));
                    todo_stack.push_front(CompilationTodo::ToCompile(ap.r, added_offset));
                }
                e => todo!("unable to compile {:?} yet", e),
            },
            CompilationTodo::Done(i) => code.push(i),
        }
    }

    code.push(Instruction::Slide(env.size() + 1));
    code.push(Instruction::Unwind);

    Ok(code)
}

#[cfg(test)]
mod test {
    use crate::parser::must_lex_and_parse_sc;

    use super::*;

    fn mk_compile_sc_expected_code_test(
        t: &str,
        expected_name: ast::Name,
        expected_n_args: usize,
        expected_code: Code,
    ) {
        let ast = must_lex_and_parse_sc(t);
        let compiled = compile_sc(ast).unwrap();
        assert_eq!(
            compiled,
            CompiledSuperCombinator {
                name: expected_name,
                n_args: expected_n_args,
                code: expected_code
            }
        )
    }

    #[test]
    fn test_compile_fix() {
        mk_compile_sc_expected_code_test(
            "fix f = f (fix f)",
            ast::Name::new("fix"),
            1,
            vec![
                Instruction::Push(0),
                Instruction::PushGlobal(ast::Name::new("fix")),
                Instruction::MkAp,
                Instruction::Push(1),
                Instruction::MkAp,
                Instruction::Slide(2),
                Instruction::Unwind,
            ],
        )
    }

    #[test]
    fn test_compile_k() {
        mk_compile_sc_expected_code_test(
            "const a b = a",
            ast::Name::new("const"),
            2,
            vec![
                Instruction::Push(0),
                Instruction::Slide(3),
                Instruction::Unwind,
            ],
        );
    }

    #[test]
    fn test_compile_s() {
        mk_compile_sc_expected_code_test(
            "s f g x = f x (g x)",
            ast::Name::new("s"),
            3,
            vec![
                Instruction::Push(2),
                Instruction::Push(2),
                Instruction::MkAp,
                Instruction::Push(3),
                Instruction::Push(2),
                Instruction::MkAp,
                Instruction::MkAp,
                Instruction::Slide(4),
                Instruction::Unwind,
            ],
        );
    }

    #[test]
    fn test_compile_add() {
        mk_compile_sc_expected_code_test(
            "two = 1 + 1",
            ast::Name::new("two"),
            0,
            vec![
                Instruction::PushNum(1),
                Instruction::PushNum(1),
                Instruction::PushGlobal(ast::Name::new("_prim_add")),
                Instruction::MkAp,
                Instruction::MkAp,
                Instruction::Slide(1),
                Instruction::Unwind,
            ],
        )
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
