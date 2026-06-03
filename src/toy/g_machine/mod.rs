mod compiler;
mod prelude;
mod types;

// use std::{
//     collections::{BTreeMap, LinkedList},
//     iter, mem,
//     rc::Rc,
// };

// mod compiler;
// mod instruction;

// use anyhow::{Context, Ok, Result, anyhow, bail};
// use derive_getters::Getters;
// use intmap::IntMap;
// use itertools::Itertools;
// use monoid::Monoid;

// use crate::{
//     parser::{
//         PRIM_ADD_NAME,
//         ast::{self, Program},
//         must_lex_and_parse_sc,
//     },
//     utils::{
//         heap::{Addr, Heap},
//         stack::Stack,
//     },
// };

// type Assoc<K, V> = BTreeMap<K, V>;

// #[derive(Debug, PartialEq, Eq, Clone)]
// enum Instruction {
//     Unwind,
//     PushGlobal(ast::Name),
//     PushNum(i64),
//     Push(usize),
//     MkAp,
//     Update(usize),
//     Pop(usize),
//     Alloc(usize),
//     Slide(usize),
//     Eval,
//     Add,
//     Branch(Box<Code>, Box<Code>),
// }

// type Code = Vec<Instruction>;

// #[derive(Debug, Clone, PartialEq, Eq)]
// enum Node {
//     Num(i64),
//     Ap(Addr, Addr),
//     Global(usize, Rc<Code>),
//     Indirect(Addr),
// }

// #[derive(Debug, Clone, Getters)]
// struct Stats {
//     #[getter(copy)]
//     steps: usize,
// }

// impl Stats {
//     fn new() -> Self {
//         Stats { steps: 0 }
//     }

//     fn incr_steps(&mut self) {
//         self.steps += 1;
//     }
// }

// #[derive(Debug)]
// enum InstrPtrNext {
//     Stay,
//     Advance,
// }

// #[derive(Debug, Clone)]
// struct DumpElem {
//     code: Rc<Code>,
//     instr_ptr: usize,
//     stack: Stack<Addr>,
// }

// #[derive(Debug, Clone, Getters)]
// pub struct Machine {
//     #[getter(skip)]
//     code: Rc<Code>,
//     #[getter(skip)]
//     instr_ptr: usize,
//     #[getter(skip)]
//     stack: Stack<Addr>,
//     #[getter(skip)]
//     dump: LinkedList<DumpElem>,
//     #[getter(skip)]
//     heap: Heap<Node>,
//     #[getter(skip)]
//     globals: Assoc<ast::Name, Addr>,
//     #[getter(skip)]
//     integers: IntMap<i64, Addr>,
//     stats: Stats,
// }

// pub enum MachineHistoryIter {
//     Machine(Machine),
//     ErrOccurred(anyhow::Error),
//     Done,
// }

// impl Iterator for MachineHistoryIter {
//     type Item = Result<Machine>;

//     fn next(&mut self) -> Option<Self::Item> {
//         match self {
//             MachineHistoryIter::Machine(machine) => {
//                 let res = machine.clone();

//                 if machine.is_done() {
//                     *self = Self::Done
//                 } else if let Err(err) = machine.step() {
//                     *self = Self::ErrOccurred(err);
//                 };

//                 Some(Ok(res))
//             }
//             MachineHistoryIter::ErrOccurred(err) => {
//                 let err = mem::replace(err, anyhow::Error::msg("dummy"));
//                 let res = Some(Err(err));
//                 *self = Self::Done;
//                 res
//             }
//             MachineHistoryIter::Done => None,
//         }
//     }
// }

// impl MachineHistoryIter {
//     fn new(machine: Machine) -> Self {
//         Self::Machine(machine)
//     }
// }

// impl Machine {
//     pub fn new(program: CompiledProgram) -> Result<Self> {
//         let (heap, globals) = build_initial_heap(program)?;
//         Ok(Self {
//             code: Rc::new(vec![]),
//             instr_ptr: 0,
//             stack: Stack::new(),
//             dump: LinkedList::new(),
//             heap,
//             globals,
//             integers: IntMap::new(),
//             stats: Stats::new(),
//         })
//     }

//     pub(super) fn is_done(&self) -> bool {
//         self.instr_ptr >= self.code.len()
//     }

//     fn load_code(&mut self, code: Rc<Code>) {
//         self.code = code;
//         self.instr_ptr = 0;
//     }

//     pub fn setup_to_run(&mut self, entry_point: ast::Name) {
//         self.load_code(Rc::new(vec![
//             Instruction::PushGlobal(entry_point),
//             Instruction::Eval,
//         ]));
//         self.stack.reset();
//         self.stats = Stats::new();
//     }

//     fn do_admin(&mut self) {
//         self.stats.incr_steps();
//     }

//     pub(super) fn step(&mut self) -> Result<()> {
//         let code = self.code.clone();
//         let instr = code.get(self.instr_ptr).unwrap();
//         let next = self.dispatch(instr)?;
//         match next {
//             InstrPtrNext::Stay => (),
//             InstrPtrNext::Advance => self.instr_ptr += 1,
//         }
//         self.do_admin();
//         Ok(())
//     }

//     fn dispatch(&mut self, i: &Instruction) -> Result<InstrPtrNext> {
//         match i {
//             Instruction::Unwind => self.handle_unwind().context("Unwind"),
//             Instruction::PushGlobal(name) => self.handle_push_global(name).context("PushGlobal"),
//             Instruction::PushNum(i) => self.handle_push_num(*i).context("PushNum"),
//             Instruction::Push(n) => self.handle_push(*n).context("Push"),
//             Instruction::MkAp => self.handle_mk_ap().context("MkAp"),
//             Instruction::Update(n) => self.handle_update(*n).context("Update"),
//             Instruction::Pop(n) => self.handle_pop(*n).context("Pop"),
//             Instruction::Alloc(n) => self.handle_alloc(*n).context("Alloc"),
//             Instruction::Slide(n) => self.handle_slide(*n).context("Slide"),
//             Instruction::Eval => self.handle_eval(),
//             Instruction::Add => self.handle_add(),
//             Instruction::Branch(instructions, instructions1) => todo!(),
//         }
//     }

//     fn handle_push_global(&mut self, name: &ast::Name) -> Result<InstrPtrNext> {
//         let addr = self.lookup_global(name)?;
//         self.stack.push(addr);
//         Ok(InstrPtrNext::Advance)
//     }

//     fn handle_push_num(&mut self, i: i64) -> Result<InstrPtrNext> {
//         let addr = self.alloc_num_node(i);
//         self.stack.push(addr);
//         Ok(InstrPtrNext::Advance)
//     }

//     fn handle_mk_ap(&mut self) -> Result<InstrPtrNext> {
//         let l = self
//             .stack
//             .pop()
//             .copied()
//             .ok_or(anyhow!("l ptr not found"))?;
//         let r = self
//             .stack
//             .pop()
//             .copied()
//             .ok_or(anyhow!("r ptr not found"))?;
//         let node = Node::Ap(l, r);
//         let addr = self.heap.alloc(node);
//         self.stack.push(addr);
//         Ok(InstrPtrNext::Advance)
//     }

//     fn handle_push(&mut self, n: usize) -> Result<InstrPtrNext> {
//         let addr = self.stack.peak_nth_from_top_cloned(n).expect(&format!(
//             "BUG: not enough args, expected {} which is greater than the stack size {}",
//             n,
//             self.stack.height()
//         ));
//         self.stack.push(addr);
//         Ok(InstrPtrNext::Advance)
//     }

//     fn handle_update(&mut self, n: usize) -> Result<InstrPtrNext> {
//         let root_addr = self.stack.pop_cloned().expect("BUG: stack is empty");
//         let root_addr = self.follow_indirect(root_addr);
//         let node_to_update_addr = self
//             .stack
//             .peak_nth_from_top_cloned(n)
//             .expect("BUG: unable to find addr of node to be updated");

//         if node_to_update_addr == root_addr {
//             bail!("infinite loop: {:?}", root_addr)
//         }

//         self.replace_node_at(node_to_update_addr, Node::Indirect(root_addr));

//         Ok(InstrPtrNext::Advance)
//     }

//     fn handle_pop(&mut self, n: usize) -> Result<InstrPtrNext> {
//         if n > 0 {
//             let n_popped = self.stack.pop_n(n).len();
//             assert_eq!(
//                 n_popped, n,
//                 "BUG: not enough args: expected {}, got {}. This should have been caught by Update",
//                 n, n_popped
//             )
//         }

//         Ok(InstrPtrNext::Advance)
//     }

//     fn handle_unwind(&mut self) -> Result<InstrPtrNext> {
//         let addr = self
//             .stack
//             .peak()
//             .copied()
//             .expect("COMPILER BUG: trying to unwind an empty stack");

//         Ok(match self.must_access_node(addr) {
//             Node::Num(_) => {
//                 if self.restore_context() {
//                     self.stack.push(addr);
//                     InstrPtrNext::Stay
//                 } else {
//                     InstrPtrNext::Advance
//                 }
//             }
//             Node::Ap(l, _) => {
//                 self.stack.push(*l);
//                 InstrPtrNext::Stay
//             }
//             Node::Global(n_args, code) => {
//                 let n_args = *n_args;
//                 let code = code.clone();

//                 if n_args > 0 {
//                     assert_eq!(self.stack.pop_cloned().unwrap(), addr);

//                     let ap_node_addrs = self.stack.pop_n_cloned(n_args);
//                     let n_args_got = ap_node_addrs.len();
//                     if n_args_got != n_args {
//                         bail!("not enough args: expected {}, got {}", n_args, n_args_got)
//                     }
//                     let last_ap_node_addr = ap_node_addrs.last().copied().unwrap();
//                     self.stack.push(last_ap_node_addr);
//                     ap_node_addrs.into_iter().rev().for_each(|addr| {
//                         let r_addr = self.must_extract_ap_node_r(addr);
//                         self.stack.push(r_addr);
//                     });
//                 }

//                 self.load_code(code);
//                 InstrPtrNext::Stay
//             }
//             Node::Indirect(indirect_addr) => {
//                 let indirect_addr = *indirect_addr;
//                 assert_eq!(self.stack.pop_cloned().unwrap(), addr);
//                 self.stack.push(indirect_addr);
//                 InstrPtrNext::Stay
//             }
//         })
//     }

//     fn handle_alloc(&mut self, n: usize) -> Result<InstrPtrNext> {
//         (0..n).try_for_each(|_| {
//             let addr = self.heap.alloc(Node::Indirect(Addr::null()));
//             self.stack.push(addr);
//             Ok(())
//         })?;

//         Ok(InstrPtrNext::Advance)
//     }

//     fn handle_slide(&mut self, n: usize) -> Result<InstrPtrNext> {
//         let addr = self.stack.pop_cloned().expect("BUG: slide on empty stack");
//         let n_popped = self.stack.pop_n(n).len();
//         assert_eq!(
//             n_popped, n,
//             "BUG: attempted to slide {}, but only got {} on the stack",
//             n, n_popped
//         );
//         self.stack.push(addr);
//         Ok(InstrPtrNext::Advance)
//     }

//     fn handle_eval(&mut self) -> Result<InstrPtrNext> {
//         let addr = self
//             .stack
//             .pop_cloned()
//             .expect("BUG: attempted to evaluate an empty stack");
//         self.save_context();
//         self.load_code(Rc::new(vec![Instruction::Unwind]));
//         self.stack.push(addr);
//         Ok(InstrPtrNext::Stay)
//     }

//     fn handle_add(&mut self) -> Result<InstrPtrNext> {
//         let l = self
//             .stack
//             .pop_cloned()
//             .expect("BUG: unable to extract l while executing add instruction");
//         let r = self
//             .stack
//             .pop_cloned()
//             .expect("BUG: unable to extract r while executing add instruction");

//         let res = match (self.must_access_node(l), self.must_access_node(r)) {
//             (Node::Num(l), Node::Num(r)) => l + r,
//             _ => panic!("BUG: add expect the two operands to both be in WHNF"),
//         };

//         let addr = self.alloc_num_node(res);
//         self.stack.push(addr);
//         Ok(InstrPtrNext::Advance)
//     }

//     fn must_extract_ap_node_r(&self, addr: Addr) -> Addr {
//         let node = self.must_access_node(addr);
//         let r = if let Node::Ap(_, r) = node {
//             *r
//         } else {
//             panic!(
//                 "COMPILER BUG: expects an Ap node at {:?}, got {:?}",
//                 addr, node
//             )
//         };
//         r
//     }

//     fn must_access_node(&self, addr: Addr) -> &Node {
//         self.heap
//             .access(addr)
//             .expect(&format!("cannot access node at {:?}", addr))
//     }

//     fn must_access_node_mut(&mut self, addr: Addr) -> &mut Node {
//         self.heap.access_mut(addr).unwrap()
//     }

//     fn lookup_global(&self, name: &ast::Name) -> Result<Addr> {
//         self.globals
//             .get(name)
//             .map(|x| *x)
//             .ok_or(anyhow!("global not found: {:?}", name))
//     }

//     fn alloc_num_node(&mut self, i: i64) -> Addr {
//         match self.integers.get(i) {
//             Some(addr) => *addr,
//             None => {
//                 let addr = self.heap.alloc(Node::Num(i));
//                 self.integers.insert(i, addr);
//                 addr
//             }
//         }
//     }

//     fn replace_node_at(&mut self, a: Addr, node: Node) {
//         *self.must_access_node_mut(a) = node;
//     }

//     fn follow_indirect(&mut self, a: Addr) -> Addr {
//         let mut next = a;
//         while let Node::Indirect(a) = self.must_access_node(next) {
//             next = *a
//         }
//         next
//     }

//     fn save_context(&mut self) {
//         let code = mem::replace(&mut self.code, Rc::new(vec![]));
//         let instr_ptr = mem::replace(&mut self.instr_ptr, 0) + 1;
//         let stack = mem::replace(&mut self.stack, Stack::new());

//         let dump_elem = DumpElem {
//             code,
//             instr_ptr,
//             stack,
//         };

//         self.dump.push_front(dump_elem);
//     }

//     fn restore_context(&mut self) -> bool {
//         if let Some(DumpElem {
//             code,
//             instr_ptr,
//             stack,
//         }) = self.dump.pop_front()
//         {
//             self.code = code;
//             self.instr_ptr = instr_ptr;
//             self.stack = stack;
//             true
//         } else {
//             false
//         }
//     }

//     pub fn history(self) -> MachineHistoryIter {
//         MachineHistoryIter::new(self)
//     }
// }

// fn prelude_ski() -> ast::Program<ast::Name> {
//     ast::Program(vec![
//         must_lex_and_parse_sc("i x = x"),
//         must_lex_and_parse_sc("k x y = x"),
//         must_lex_and_parse_sc("s f g x = f x (g x)"),
//     ])
// }

// fn compiled_prelude_ski() -> CompiledProgram {
//     compile_program(prelude_ski()).unwrap()
// }

// fn compiled_primitive_wrappers() -> CompiledProgram {
//     CompiledProgram(vec![CompiledSuperCombinator {
//         name: ast::Name::new(PRIM_ADD_NAME),
//         n_args: 2,
//         code: vec![
//             Instruction::Push(0),
//             Instruction::Eval,
//             Instruction::Push(2),
//             Instruction::Eval,
//             Instruction::Add,
//             Instruction::Update(2),
//             Instruction::Pop(2),
//             Instruction::Unwind,
//         ],
//     }])
// }

// fn build_initial_heap(program: CompiledProgram) -> Result<(Heap<Node>, Assoc<ast::Name, Addr>)> {
//     Ok(program.0.into_iter().fold(
//         (Heap::new(), Assoc::new()),
//         |(mut heap, mut globals), sc| {
//             let addr = heap.alloc(Node::Global(sc.n_args, Rc::new(sc.code)));
//             globals.insert(sc.name, addr);
//             (heap, globals)
//         },
//     ))
// }

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct CompiledProgram(Vec<CompiledSuperCombinator>);

// impl Monoid for CompiledProgram {
//     fn id() -> Self {
//         Self(vec![])
//     }

//     fn op(self, other: Self) -> Self {
//         Self(
//             self.0
//                 .into_iter()
//                 .chain(other.0)
//                 .dedup_by(|l, r| l.name == r.name)
//                 .collect(),
//         )
//     }
// }

// pub fn compile_program_with_prelude(prog: Program<ast::Name>) -> Result<CompiledProgram> {
//     compile_program(prog).map(|c| {
//         c.op(compiled_prelude_ski())
//             .op(compiled_primitive_wrappers())
//     })
// }

// fn compile_program(prog: Program<ast::Name>) -> Result<CompiledProgram> {
//     Ok(CompiledProgram(
//         prog.0
//             .into_iter()
//             .dedup_by(|l, r| l.name == r.name)
//             .map(compile_sc)
//             .collect::<Result<Vec<CompiledSuperCombinator>>>()?
//             .into_iter()
//             .collect(),
//     ))
// }

// #[derive(Debug, Clone, PartialEq, Eq)]
// struct CompiledSuperCombinator {
//     name: ast::Name,
//     n_args: usize,
//     code: Code,
// }

// fn compile_sc(sc: ast::SuperCombinator<ast::Name>) -> Result<CompiledSuperCombinator> {
//     let env: Assoc<Rc<ast::Name>, usize> = sc
//         .arguments
//         .into_iter()
//         .enumerate()
//         .map(|(x, y)| (Rc::new(y), x))
//         .collect();
//     let n_env = env.len();
//     let code = compile_expr(sc.body, Rc::new(env))?;
//     Ok(CompiledSuperCombinator {
//         name: sc.name,
//         n_args: n_env,
//         code,
//     })
// }

// #[derive(Debug, Clone)]
// enum CompilationTodo {
//     ToCompile(ast::Expr<ast::Name>, Rc<Assoc<Rc<ast::Name>, usize>>),
//     Done(Instruction),
// }

// fn env_offset_by(n: usize, env: &mut Assoc<Rc<ast::Name>, usize>) {
//     env.values_mut().for_each(|offset| *offset += n);
// }

// fn compile_expr(e: ast::Expr<ast::Name>, rc_env: Rc<Assoc<Rc<ast::Name>, usize>>) -> Result<Code> {
//     let n_args = rc_env.len();
//     let mut todo_stack = LinkedList::<CompilationTodo>::new();
//     let mut code = Code::new();

//     todo_stack.push_front(CompilationTodo::ToCompile(e, rc_env));

//     while let Some(todo) = todo_stack.pop_front() {
//         match todo {
//             CompilationTodo::ToCompile(expr, env) => match expr {
//                 ast::Expr::Var(name) => todo_stack.push_front(CompilationTodo::Done(
//                     env.get(&name)
//                         .map_or(Instruction::PushGlobal(name), |offset| {
//                             Instruction::Push(*offset)
//                         }),
//                 )),
//                 ast::Expr::Num(i) => {
//                     todo_stack.push_front(CompilationTodo::Done(Instruction::PushNum(i.0)))
//                 }
//                 ast::Expr::Ap(ap) => {
//                     let ap = *ap;
//                     let r_env = env.clone();
//                     let l_env = {
//                         let mut env = Rc::unwrap_or_clone(env);
//                         env_offset_by(1, &mut env);
//                         Rc::new(env)
//                     };
//                     [
//                         CompilationTodo::ToCompile(ap.r, r_env),
//                         CompilationTodo::ToCompile(ap.l, l_env),
//                         CompilationTodo::Done(Instruction::MkAp),
//                     ]
//                     .into_iter()
//                     .rev()
//                     .for_each(|todo| todo_stack.push_front(todo));
//                 }
//                 ast::Expr::Let(l) => {
//                     let l = *l;

//                     let n_binds = l.definitions.len();
//                     let pre_alloc_todo = CompilationTodo::Done(Instruction::Alloc(n_binds));

//                     let env = Rc::unwrap_or_clone(env);

//                     let (def_todos, rc_env) = if l.is_recursive {
//                         let mut env = env;
//                         env_offset_by(n_binds, &mut env);

//                         let (extra_env, def_body_update_offset_pairs): (Vec<_>, Vec<_>) = l
//                             .definitions
//                             .into_iter()
//                             .rev()
//                             .enumerate()
//                             .map(|(idx, def)| ((Rc::new(def.binder), idx), (def.body, idx)))
//                             .unzip();

//                         let duplicated_names = extra_env
//                             .iter()
//                             .counts_by(|(n, _)| n.clone())
//                             .into_iter()
//                             .filter_map(|(name, count)| (count > 1).then_some(name))
//                             .collect::<Vec<_>>();
//                         if !duplicated_names.is_empty() {
//                             let duplicated_names = duplicated_names
//                                 .iter()
//                                 .map(|name| name.0.as_str())
//                                 .join(", ");
//                             bail!("found duplicate binders in letrec: {}", duplicated_names)
//                         }

//                         env.extend(extra_env);
//                         let rc_env = Rc::new(env);

//                         let def_todos = iter::once(pre_alloc_todo)
//                             .chain(def_body_update_offset_pairs.into_iter().flat_map(
//                                 |(body, update_offset)| {
//                                     [
//                                         CompilationTodo::ToCompile(body, rc_env.clone()),
//                                         CompilationTodo::Done(Instruction::Update(update_offset)),
//                                     ]
//                                     .into_iter()
//                                 },
//                             ))
//                             .collect::<Vec<_>>();

//                         (def_todos, rc_env)
//                     } else {
//                         let mut env = env;
//                         let def_todos = l
//                             .definitions
//                             .into_iter()
//                             .scan(&mut env, |env, def| {
//                                 let res =
//                                     CompilationTodo::ToCompile(def.body, Rc::new(env.clone()));
//                                 env_offset_by(1, env);
//                                 env.insert(Rc::new(def.binder), 0);
//                                 Some(res)
//                             })
//                             .collect::<Vec<_>>();

//                         (def_todos, Rc::new(env))
//                     };

//                     let body_todo = CompilationTodo::ToCompile(l.body, rc_env);

//                     def_todos
//                         .into_iter()
//                         .chain(iter::once(body_todo))
//                         .chain(iter::once(CompilationTodo::Done(Instruction::Slide(
//                             n_binds,
//                         ))))
//                         .rev()
//                         .for_each(|todo| todo_stack.push_front(todo));
//                 }
//                 e => todo!("unable to compile {:?} yet", e),
//             },
//             CompilationTodo::Done(i) => code.push(i),
//         }
//     }

//     code.push(Instruction::Update(n_args));
//     code.push(Instruction::Pop(n_args));
//     code.push(Instruction::Unwind);

//     Ok(code)
// }

// #[cfg(test)]
// mod test_machine {
//     use super::*;
//     use crate::parser::must_lex_and_parse_sc;

//     fn run_expr(expr: &str) -> Machine {
//         let entry_point = "main";
//         let program = format!("{} = {}", entry_point, expr);
//         let program = ast::Program(vec![must_lex_and_parse_sc(program)]);
//         let compiled_program = compile_program_with_prelude(program).unwrap();
//         let mut machine = Machine::new(compiled_program).unwrap();
//         machine.setup_to_run(ast::Name::new(entry_point));
//         let h = machine.history();
//         let machine_end = h
//             .map(|m| {
//                 eprintln!("{:?}\n", m);
//                 m
//             })
//             .last()
//             .unwrap()
//             .unwrap();
//         machine_end
//     }

//     fn assert_machine_done(machine_end: Machine, expected_top_node: Node) {
//         assert_eq!(machine_end.stack.height(), 1);
//         let top_addr = machine_end.stack.peak().unwrap();
//         let top_node = machine_end.must_access_node(*top_addr);
//         assert_eq!(top_node, &expected_top_node);
//         eprintln!("stats: {:?}", machine_end.stats());
//     }

//     #[test]
//     fn test_add() {
//         assert_machine_done(run_expr("i 1 + s k k 1 + 40"), Node::Num(42));
//     }

//     #[test]
//     fn test_ski() {
//         assert_machine_done(run_expr("s k k (i 3)"), Node::Num(3));
//     }
// }

// #[cfg(test)]
// mod test_compilation {
//     use crate::parser::must_lex_and_parse_sc;

//     use super::*;

//     fn mk_compile_sc_expected_code_test(
//         t: &str,
//         expected_name: ast::Name,
//         expected_n_args: usize,
//         expected_code: Code,
//     ) {
//         let ast = must_lex_and_parse_sc(t);
//         let compiled = compile_sc(ast).unwrap();
//         assert_eq!(
//             compiled,
//             CompiledSuperCombinator {
//                 name: expected_name,
//                 n_args: expected_n_args,
//                 code: expected_code
//             }
//         )
//     }

//     #[test]
//     fn test_compile_letrec_fix() {
//         mk_compile_sc_expected_code_test(
//             "fix f = letrec x = f x in x",
//             ast::Name::new("fix"),
//             1,
//             vec![
//                 Instruction::Alloc(1),
//                 Instruction::Push(0),
//                 Instruction::Push(2),
//                 Instruction::MkAp,
//                 Instruction::Update(0),
//                 Instruction::Push(0),
//                 Instruction::Slide(1),
//                 Instruction::Update(1),
//                 Instruction::Pop(1),
//                 Instruction::Unwind,
//             ],
//         )
//     }

//     #[test]
//     fn test_compile_let() {
//         mk_compile_sc_expected_code_test(
//             "f = let x = 0; y = x in x + y",
//             ast::Name::new("f"),
//             0,
//             vec![
//                 Instruction::PushNum(0),
//                 Instruction::Push(0),
//                 Instruction::Push(0),
//                 Instruction::Push(2),
//                 Instruction::PushGlobal(ast::Name::new("_prim_add")),
//                 Instruction::MkAp,
//                 Instruction::MkAp,
//                 Instruction::Slide(2),
//                 Instruction::Update(0),
//                 Instruction::Pop(0),
//                 Instruction::Unwind,
//             ],
//         )
//     }

//     #[test]
//     fn test_compile_fix() {
//         mk_compile_sc_expected_code_test(
//             "fix f = f (fix f)",
//             ast::Name::new("fix"),
//             1,
//             vec![
//                 Instruction::Push(0),
//                 Instruction::PushGlobal(ast::Name::new("fix")),
//                 Instruction::MkAp,
//                 Instruction::Push(1),
//                 Instruction::MkAp,
//                 Instruction::Update(1),
//                 Instruction::Pop(1),
//                 Instruction::Unwind,
//             ],
//         )
//     }

//     #[test]
//     fn test_compile_k() {
//         mk_compile_sc_expected_code_test(
//             "const a b = a",
//             ast::Name::new("const"),
//             2,
//             vec![
//                 Instruction::Push(0),
//                 Instruction::Update(2),
//                 Instruction::Pop(2),
//                 Instruction::Unwind,
//             ],
//         );
//     }

//     #[test]
//     fn test_compile_s() {
//         mk_compile_sc_expected_code_test(
//             "s f g x = f x (g x)",
//             ast::Name::new("s"),
//             3,
//             vec![
//                 Instruction::Push(2),
//                 Instruction::Push(2),
//                 Instruction::MkAp,
//                 Instruction::Push(3),
//                 Instruction::Push(2),
//                 Instruction::MkAp,
//                 Instruction::MkAp,
//                 Instruction::Update(3),
//                 Instruction::Pop(3),
//                 Instruction::Unwind,
//             ],
//         );
//     }

//     #[test]
//     fn test_compile_add() {
//         mk_compile_sc_expected_code_test(
//             "two = 1 + 1",
//             ast::Name::new("two"),
//             0,
//             vec![
//                 Instruction::PushNum(1),
//                 Instruction::PushNum(1),
//                 Instruction::PushGlobal(ast::Name::new("_prim_add")),
//                 Instruction::MkAp,
//                 Instruction::MkAp,
//                 Instruction::Update(0),
//                 Instruction::Pop(0),
//                 Instruction::Unwind,
//             ],
//         )
//     }
// }

// #[cfg(test)]
// mod test_misc {
//     use super::*;

//     #[test]
//     fn test_dedup() {
//         let v = [(ast::Name::new("i"), 1), (ast::Name::new("i"), 2usize)]
//             .into_iter()
//             .dedup_by(|l, r| l.0 == r.0)
//             .map(|(_, r)| r)
//             .collect::<Vec<_>>();
//         assert_eq!(v, vec![1]);
//     }
// }

// pub mod postfix_eval {
//     use std::collections::LinkedList;

//     use chumsky::container::Container;

//     #[derive(Debug, Clone)]
//     pub enum Expr {
//         Num(i64),
//         Plus(Box<Expr>, Box<Expr>),
//         Mul(Box<Expr>, Box<Expr>),
//     }

//     pub fn tree_eval(expr: Expr) -> i64 {
//         match expr {
//             Expr::Num(i) => i,
//             Expr::Plus(l, r) => tree_eval(*l) + tree_eval(*r),
//             Expr::Mul(l, r) => tree_eval(*l) * tree_eval(*r),
//         }
//     }

//     #[derive(Debug)]
//     pub enum Instruction {
//         Num(i64),
//         Plus,
//         Mul,
//     }

//     #[derive(Debug)]
//     pub struct Machine {
//         instructions: LinkedList<Instruction>,
//         stack: LinkedList<i64>,
//     }

//     impl Machine {
//         pub fn new(instrs: Vec<Instruction>) -> Self {
//             Self {
//                 instructions: instrs.into_iter().collect(),
//                 stack: LinkedList::new(),
//             }
//         }

//         pub fn execute(mut self) -> i64 {
//             self.run();
//             self.stack.pop_back().unwrap()
//         }

//         fn step(&mut self, i: Instruction) {
//             match i {
//                 Instruction::Num(i) => self.stack.push_front(i),
//                 Instruction::Plus => {
//                     let l = self.stack.pop_front().unwrap();
//                     let r = self.stack.pop_front().unwrap();
//                     self.stack.push_front(l + r)
//                 }
//                 Instruction::Mul => {
//                     let l = self.stack.pop_front().unwrap();
//                     let r = self.stack.pop_front().unwrap();
//                     self.stack.push_front(l * r)
//                 }
//             }
//         }

//         fn run(&mut self) {
//             while let Some(instr) = self.instructions.pop_front() {
//                 self.step(instr);
//             }
//         }
//     }

//     #[derive(Debug)]
//     enum Imm {
//         ToCompile(Expr),
//         Compiled(Instruction),
//     }

//     pub fn compile(expr: Expr) -> Vec<Instruction> {
//         let mut imm_stack = LinkedList::<Imm>::new();
//         let mut instrs = Vec::new();
//         imm_stack.push(Imm::ToCompile(expr));
//         while let Some(imm) = imm_stack.pop_front() {
//             match imm {
//                 Imm::ToCompile(expr) => match expr {
//                     Expr::Num(i) => imm_stack.push_front(Imm::Compiled(Instruction::Num(i))),
//                     Expr::Plus(l, r) => {
//                         imm_stack.push_front(Imm::Compiled(Instruction::Plus));
//                         imm_stack.push_front(Imm::ToCompile(*r));
//                         imm_stack.push_front(Imm::ToCompile(*l));
//                     }
//                     Expr::Mul(l, r) => {
//                         imm_stack.push_front(Imm::Compiled(Instruction::Mul));
//                         imm_stack.push_front(Imm::ToCompile(*r));
//                         imm_stack.push_front(Imm::ToCompile(*l));
//                     }
//                 },
//                 Imm::Compiled(i) => instrs.push(i),
//             }
//         }
//         instrs
//     }

//     #[cfg(test)]
//     mod test {
//         use super::*;

//         #[test]
//         fn test() {
//             let expr = Expr::Plus(
//                 Box::new(Expr::Num(2)),
//                 Box::new(Expr::Mul(Box::new(Expr::Num(3)), Box::new(Expr::Num(4)))),
//             );
//             let postfix_eval_result = Machine::new(compile(expr.clone())).execute();
//             let tree_eval_result = tree_eval(expr);
//             assert_eq!(postfix_eval_result, tree_eval_result)
//         }
//     }
// }
