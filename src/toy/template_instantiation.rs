use core::panic;
use std::{
    cell::RefCell,
    collections::{BTreeMap, LinkedList},
    mem,
    ops::Deref,
    rc::Rc,
};

use itertools::{Either, Itertools};

use crate::parser::{
    ast, must_lex_and_parse_sc, prelude, PRIM_ADD_NAME, PRIM_BOOLEAN_AND_NAME,
    PRIM_BOOLEAN_OR_NAME, PRIM_DIV_NAME, PRIM_EQ_NAME, PRIM_GE_NAME, PRIM_GT_NAME, PRIM_LE_NAME,
    PRIM_LT_NAME, PRIM_MUL_NAME, PRIM_NE_NAME, PRIM_SUB_NAME,
};

#[derive(Debug, Clone)]
pub struct Stack<T>(pub LinkedList<T>);

impl<T> Stack<T> {
    pub fn new() -> Stack<T> {
        Stack(LinkedList::new())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn push(&mut self, e: T) {
        self.0.push_front(e);
    }

    pub fn pop(&mut self) -> Option<T> {
        self.0.pop_front()
    }

    pub fn peak(&self) -> Option<&T> {
        self.0.front()
    }

    pub fn pop_n_releaxed(&mut self, n: usize) -> Vec<T> {
        (0..n).into_iter().map_while(|_| self.pop()).collect()
    }

    pub fn peak_n_releaxed(&self, n: usize) -> Vec<&T> {
        self.0.iter().take(n).collect()
    }
}

#[derive(Debug, Clone)]
pub struct Heap<T>(pub Vec<T>);

impl<T> Heap<T> {
    pub fn new() -> Heap<T> {
        Heap(Vec::new())
    }

    pub fn alloc(&mut self, v: T) -> Addr {
        let addr = Addr::new(self.0.len());
        self.0.push(v);
        addr
    }

    pub fn access(&self, idx: Addr) -> Option<&T> {
        self.0.get(idx.0)
    }

    pub fn access_mut(&mut self, idx: Addr) -> Option<&mut T> {
        self.0.get_mut(idx.0)
    }
}

#[derive(Debug, Clone)]
pub struct Assoc<K, V>(pub BTreeMap<K, V>);

impl<K: Ord, V> Assoc<K, V> {
    pub fn new() -> Assoc<K, V> {
        Assoc(BTreeMap::new())
    }

    pub fn insert(&mut self, k: K, v: V) {
        let _ = self.0.insert(k, v);
    }

    pub fn lookup(&self, k: &K) -> Option<&V> {
        self.0.get(k)
    }

    // Right-bias
    pub fn combine(l: Assoc<K, V>, r: Assoc<K, V>) -> Assoc<K, V> {
        Assoc(l.0.into_iter().chain(r.0.into_iter()).collect())
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Addr(pub usize);

impl Addr {
    fn new(idx: usize) -> Addr {
        Addr(idx)
    }
}

#[derive(Debug, Clone)]
pub struct ApplicationNode {
    pub l: Addr,
    pub r: Addr,
}

impl ApplicationNode {
    pub fn new(l: Addr, r: Addr) -> Self {
        Self { l, r }
    }
}

#[derive(Debug, Clone)]
pub struct SuperCombinatorNode(pub ast::SuperCombinator<ast::Name>);

impl SuperCombinatorNode {
    fn new(sc: ast::SuperCombinator<ast::Name>) -> SuperCombinatorNode {
        SuperCombinatorNode(sc)
    }
}

#[derive(Debug, Clone)]
pub struct IntegerNode(pub i64);

impl IntegerNode {
    fn new(i: i64) -> IntegerNode {
        IntegerNode(i)
    }
}

custom_derive! {
    #[derive(Debug, Copy, Clone, IterVariants(PrimOpKindVariants))]
    pub enum PrimOpKind {
        Neg,
        Add,
        Sub,
        Mul,
        Div,
        Eq,
        Lt,
        Gt,
        IfThenElse,
        Constr
    }
}

impl PrimOpKind {
    fn to_name(&self) -> Option<&'static str> {
        match self {
            PrimOpKind::Neg => Some("_prim_neg"),
            PrimOpKind::Add => Some(PRIM_ADD_NAME),
            PrimOpKind::Sub => Some(PRIM_SUB_NAME),
            PrimOpKind::Mul => Some(PRIM_MUL_NAME),
            PrimOpKind::Div => Some(PRIM_DIV_NAME),
            PrimOpKind::Eq => Some(PRIM_EQ_NAME),
            PrimOpKind::Lt => Some(PRIM_LT_NAME),
            PrimOpKind::Gt => Some(PRIM_GT_NAME),
            PrimOpKind::IfThenElse => Some("_prim_if_then_else"),
            PrimOpKind::Constr => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConstrPrimOp {
    pub tag: u64,
    pub arity: usize,
}

impl ConstrPrimOp {
    pub fn new(tag: u64, arity: usize) -> Self {
        Self { tag, arity }
    }
}

#[derive(Debug, Clone)]
pub enum PrimOp {
    Neg,
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Lt,
    Gt,
    IfThenElse,
    Constr(ConstrPrimOp),
}

impl PrimOp {
    fn new_from_kind(k: PrimOpKind) -> Option<Self> {
        match k {
            PrimOpKind::Neg => Some(PrimOp::Neg),
            PrimOpKind::Add => Some(PrimOp::Add),
            PrimOpKind::Sub => Some(PrimOp::Sub),
            PrimOpKind::Mul => Some(PrimOp::Mul),
            PrimOpKind::Div => Some(PrimOp::Div),
            PrimOpKind::Eq => Some(PrimOp::Eq),
            PrimOpKind::Lt => Some(PrimOp::Lt),
            PrimOpKind::Gt => Some(PrimOp::Gt),
            PrimOpKind::IfThenElse => Some(PrimOp::IfThenElse),
            PrimOpKind::Constr => None,
        }
    }

    fn get_kind(&self) -> PrimOpKind {
        match self {
            PrimOp::Neg => PrimOpKind::Neg,
            PrimOp::Add => PrimOpKind::Add,
            PrimOp::Sub => PrimOpKind::Sub,
            PrimOp::Mul => PrimOpKind::Mul,
            PrimOp::Div => PrimOpKind::Div,
            PrimOp::Eq => PrimOpKind::Eq,
            PrimOp::Lt => PrimOpKind::Lt,
            PrimOp::Gt => PrimOpKind::Gt,
            PrimOp::IfThenElse => PrimOpKind::IfThenElse,
            PrimOp::Constr(_) => PrimOpKind::Constr,
        }
    }

    fn get_arity(&self) -> usize {
        match self {
            PrimOp::Neg => 1,
            PrimOp::Add => 2,
            PrimOp::Sub => 2,
            PrimOp::Mul => 2,
            PrimOp::Div => 2,
            PrimOp::Eq => 2,
            PrimOp::Lt => 2,
            PrimOp::Gt => 2,
            PrimOp::IfThenElse => 3,
            PrimOp::Constr(c) => c.arity as usize,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PrimNode(pub PrimOp);

impl PrimNode {
    pub fn new_from_kind(k: PrimOpKind) -> Option<Self> {
        PrimOp::new_from_kind(k).map(Self)
    }

    pub fn new(o: PrimOp) -> Self {
        Self(o)
    }
}

#[derive(Debug, Clone)]
pub struct DataNode {
    pub tag: u64,
    pub field_addrs: Vec<Addr>,
}

impl DataNode {
    pub fn new(tag: u64, field_addrs: Vec<Addr>) -> Self {
        Self { tag, field_addrs }
    }
}

#[derive(Debug, Clone)]
pub enum Node {
    Ap(ApplicationNode),
    SuperComb(SuperCombinatorNode),
    Num(IntegerNode),
    Prim(PrimNode),
    Data(DataNode),
    Indirect(Addr),
    Dummy,
}

impl Node {
    fn new_in_rc_refcell(n: Node) -> Rc<RefCell<Node>> {
        Rc::new(RefCell::new(n))
    }

    fn is_data_node(&self) -> bool {
        match self {
            Node::Num(_) => true,
            Node::Data(_) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Stats {
    pub steps: usize,
}

impl Stats {
    pub fn new() -> Stats {
        Stats { steps: 0 }
    }

    pub fn incr_steps(&mut self) {
        self.steps += 1;
    }
}

#[derive(Debug, Clone)]
pub struct Machine {
    pub stack: Stack<Addr>,
    pub dump: Stack<Stack<Addr>>,
    pub heap: Heap<Rc<RefCell<Node>>>,
    pub globals: Assoc<ast::Name, Addr>,
    pub stats: Stats,
}

pub const FALSE_TAG: u64 = 0;
pub const TRUE_TAG: u64 = 1;

fn extended_prelude() -> Vec<ast::SuperCombinator<ast::Name>> {
    vec![
        must_lex_and_parse_sc(&format!("neg = {}", PrimOpKind::Neg.to_name().unwrap())),
        must_lex_and_parse_sc(&format!("false = Pack{{{},0}}", FALSE_TAG)),
        must_lex_and_parse_sc(&format!("true = Pack{{{},0}}", TRUE_TAG)),
        must_lex_and_parse_sc(&format!(
            "if = {}",
            PrimOpKind::IfThenElse.to_name().unwrap()
        )),
        must_lex_and_parse_sc(&format!("{} x y = if x y false", PRIM_BOOLEAN_AND_NAME)),
        must_lex_and_parse_sc(&format!("{} x y = if x true y", PRIM_BOOLEAN_OR_NAME)),
        must_lex_and_parse_sc("not x = if x false true"),
        must_lex_and_parse_sc("xor x y = if x (not y) y"),
        must_lex_and_parse_sc(&format!(
            "{} x y = not ({} x y)",
            PRIM_NE_NAME, PRIM_EQ_NAME
        )),
        must_lex_and_parse_sc(&format!("{} x y = (x < y) || (x == y) ", PRIM_LE_NAME)),
        must_lex_and_parse_sc(&format!("{} x y = (x > y) || (x == y) ", PRIM_GE_NAME)),
    ]
}

fn build_initial_heap(
    scs: Vec<ast::SuperCombinator<ast::Name>>,
) -> (Heap<Rc<RefCell<Node>>>, Assoc<ast::Name, Addr>) {
    PrimOpKind::iter_variants()
        .filter_map(|op| {
            Some((
                ast::Name::new(op.to_name()?),
                Node::Prim(PrimNode::new_from_kind(op)?),
            ))
        })
        .chain(
            prelude()
                .into_iter()
                .chain(extended_prelude())
                .chain(scs)
                .map(|sc| {
                    (
                        sc.name.clone(),
                        Node::SuperComb(SuperCombinatorNode::new(sc)),
                    )
                }),
        )
        .fold(
            (Heap::new(), Assoc::new()),
            |(mut heap, mut globals), (name, node)| {
                let addr = heap.alloc(Node::new_in_rc_refcell(node));
                globals.insert(name, addr);
                (heap, globals)
            },
        )
}

impl Machine {
    pub fn new(p: ast::Program<ast::Name>) -> Machine {
        let (heap, globals) = build_initial_heap(p.0);
        let stack = Stack::new();
        let dump = Stack::new();
        let stats = Stats::new();
        Machine {
            stack,
            dump,
            heap,
            globals,
            stats,
        }
    }

    fn do_admin(&mut self) {
        self.stats.incr_steps();
    }

    pub fn eval(&mut self, entry_point: Option<&ast::Name>) -> Result<(), String> {
        let fallback_entry_point = ast::Name::new("main");
        let entry_point = entry_point.unwrap_or(&fallback_entry_point);
        let entry_point_addr = *self
            .globals
            .lookup(entry_point)
            .ok_or(format!("entry point '{:?}' not found", entry_point))?;
        self.stack.push(entry_point_addr);
        loop {
            if self.is_fully_reduce() {
                if self.dump.is_empty() {
                    break;
                }
                self.pop_stack_frame();
            } else {
                self.eval_step()?;
                self.do_admin();
            };
        }
        Ok(())
    }

    fn eval_step(&mut self) -> Result<(), String> {
        let addr = *self.stack.peak().unwrap(); // Guarded by isDone
        let node = self.heap.access(addr).unwrap().clone();
        self.dispatch(addr, node)
    }

    fn dispatch(&mut self, node_addr: Addr, node: Rc<RefCell<Node>>) -> Result<(), String> {
        match *node.borrow() {
            Node::Num(ref num_node) => self.handle_num_node(node_addr, num_node),
            Node::Data(ref data_node) => self.handle_data_node(node_addr, data_node),
            Node::Ap(ref ap_node) => self.handle_ap_node(node_addr, ap_node),
            Node::SuperComb(ref super_comb_node) => {
                self.handle_super_comb_node(node_addr, super_comb_node)
            }
            Node::Prim(ref prim_node) => self.handle_prim_node(node_addr, prim_node),
            Node::Indirect(addr) => self.handle_indirect_node(node_addr, addr),
            Node::Dummy => panic!("BUG: incomplete template instantiation results in dummy node"),
        }
    }

    fn handle_indirect_node(&mut self, _node_addr: Addr, addr: Addr) -> Result<(), String> {
        let _ = self.stack.pop();
        self.stack.push(addr);
        Ok(())
    }

    fn handle_num_node(&mut self, _node_addr: Addr, _n: &IntegerNode) -> Result<(), String> {
        Err("cannot apply to an integer".to_string())
    }

    fn handle_data_node(&mut self, _node_addr: Addr, _n: &DataNode) -> Result<(), String> {
        Err("cannot apply to a data node".to_string())
    }

    fn handle_ap_node(&mut self, node_addr: Addr, n: &ApplicationNode) -> Result<(), String> {
        let r = self.follow_indirect(n.r);
        if r != n.r {
            self.replace_node_at(node_addr, Node::Ap(ApplicationNode { l: n.l, r: r }));
        }
        self.stack.push(n.l);
        Ok(())
    }

    fn follow_indirect(&self, a: Addr) -> Addr {
        match self.heap.access(a).unwrap().borrow().deref() {
            Node::Indirect(a) => self.follow_indirect(*a),
            _ => a,
        }
    }

    fn handle_prim_node(&mut self, node_addr: Addr, prim: &PrimNode) -> Result<(), String> {
        match &prim.0 {
            PrimOp::Constr(c) => self.handle_prim_node_constr(node_addr, c),
            PrimOp::IfThenElse => self.handle_prim_node_if_then_else(node_addr),
            _ => self.handle_prim_node_nf(node_addr, prim),
        }
    }

    fn handle_prim_node_constr(
        &mut self,
        node_addr: Addr,
        constr_prim_op: &ConstrPrimOp,
    ) -> Result<(), String> {
        assert_eq!(self.stack.pop(), Some(node_addr));
        let arity = constr_prim_op.arity;
        let ap_node_addrs = self.stack.pop_n_releaxed(arity);
        let num_popped = ap_node_addrs.len();

        if num_popped != arity {
            Err(format!(
                "constructor expected {} args, got {}",
                arity, num_popped
            ))?
        }

        let node_to_override = ap_node_addrs.last().copied().unwrap_or(node_addr);

        let arg_addrs = ap_node_addrs
            .into_iter()
            .map(
                |addr| match self.heap.access(addr).unwrap().borrow().deref() {
                    Node::Ap(a) => a.r,
                    node => panic!("BUG: expected Ap node, got {:?}", node),
                },
            )
            .collect::<Vec<Addr>>();

        let data_node = Node::Data(DataNode::new(constr_prim_op.tag, arg_addrs));

        self.replace_node_at(node_to_override, data_node);

        self.stack.push(node_to_override);

        Ok(())
    }

    fn handle_prim_node_if_then_else(&mut self, node_addr: Addr) -> Result<(), String> {
        assert_eq!(self.stack.pop(), Some(node_addr));
        // stack layout deref:
        // PrimNode IfThenElse
        // Ap _l cond
        // Ap _l then_branch
        // Ap _l else_branch

        let pred_ap_addr = self.stack.pop().ok_or("cond expr missing".to_string())?;
        let pred_addr = self.must_get_application_node_r_at(pred_ap_addr);

        let then_branch_ap_addr = self.stack.pop().ok_or("then branch missing".to_string())?;
        let then_branch_addr = self.must_get_application_node_r_at(then_branch_ap_addr);
        let else_branch_ap_addr = self.stack.pop().ok_or("else branch missing".to_string())?;
        let else_branch_addr = self.must_get_application_node_r_at(else_branch_ap_addr);

        let node_addr_to_override = else_branch_ap_addr;

        let next = match self.heap.access(pred_addr).unwrap().borrow().deref() {
            Node::Num(_) => {
                // Otherwise it won't stop evaluating
                Err("predicate expression evaluated to num".to_string())?
            }
            Node::Data(d) => {
                let next_addr = match (d.tag, d.field_addrs.len()) {
                    (TRUE_TAG, 0) => Ok(then_branch_addr),
                    (FALSE_TAG, 0) => Ok(else_branch_addr),
                    (tag, fields_len) => Err(format!(
                        "predicate expression didn't evaluate to boolean, tag: {}, fields len: {}",
                        tag, fields_len
                    )),
                }?;
                Either::Right(next_addr)
            }
            _ => Either::Left(pred_addr),
        };

        match next {
            Either::Right(next_addr) => {
                self.replace_node_at(node_addr_to_override, Node::Indirect(next_addr));
                self.stack.push(node_addr_to_override);
            }
            Either::Left(pred_addr) => {
                // Predicate expression needs to be evaluated to NF first
                self.stack.push(else_branch_ap_addr);
                self.stack.push(then_branch_ap_addr);
                self.stack.push(pred_ap_addr);
                self.stack.push(node_addr);
                self.push_stack_frame();
                self.stack.push(pred_addr);
            }
        }

        Ok(())
    }

    fn handle_prim_node_nf(&mut self, _node_addr: Addr, prim: &PrimNode) -> Result<(), String> {
        let arity = prim.0.get_arity();
        let num_addrs_to_pop = arity + 1;
        let ap_node_addrs = self
            .stack
            .peak_n_releaxed(num_addrs_to_pop)
            .into_iter()
            .skip(1)
            .copied()
            .collect::<Vec<Addr>>();
        if ap_node_addrs.len() != arity {
            Err(format!(
                "prim op {:?} expected {} arguments, got {}",
                prim.0,
                arity,
                ap_node_addrs.len()
            ))?
        }
        let node_addr_to_override = *ap_node_addrs.last().unwrap();
        let (unevaluated, evaluated): (Vec<_>, Vec<_>) = ap_node_addrs
            .into_iter()
            .map(|addr| self.must_get_application_node_r_at(addr))
            .partition_map(|arg_addr| {
                let node = self.heap.access(arg_addr).unwrap().borrow();
                if node.is_data_node() {
                    // FIXME: dont think this is okay, there is no guarantee that a Data Node is fully reduced.
                    Either::Right(arg_addr)
                } else {
                    Either::Left(arg_addr)
                }
            });
        match unevaluated.into_iter().next() {
            None => {
                self.stack.pop_n_releaxed(num_addrs_to_pop);
                let node = self.run_prim_op_nf(prim.0.get_kind(), evaluated)?;
                self.replace_node_at(node_addr_to_override, node);
                self.stack.push(node_addr_to_override);
            }
            Some(addr) => {
                self.push_stack_frame();
                self.stack.push(addr);
            }
        }
        Ok(())
    }

    fn impl_prim_all_num_args<const N: usize, F>(
        &mut self,
        arg_addrs: Vec<Addr>,
        f: F,
    ) -> Result<Node, String>
    where
        F: Fn([i64; N]) -> Result<Node, String>,
    {
        let args_vec: Vec<i64> = arg_addrs
            .into_iter()
            .map(
                |addr| match self.heap.access(addr).unwrap().borrow().deref() {
                    Node::Num(n) => Ok(n.0),
                    n => Err(format!("expected integer node at {:?}, got {:?}", addr, n)),
                },
            )
            .try_collect()?;
        let args_arr: [i64; N] = args_vec
            .try_into()
            .map_err(|v: Vec<i64>| format!("expected {} args, got {}", N, v.len()))?;
        f(args_arr).map_err(|e| format!("error while executing the prim op: {}", e))
    }

    fn impl_prim_all_num_args_ret_num<const N: usize, F>(
        &mut self,
        arg_addrs: Vec<Addr>,
        f: F,
    ) -> Result<Node, String>
    where
        F: Fn([i64; N]) -> Result<i64, String>,
    {
        self.impl_prim_all_num_args(arg_addrs, |args| {
            f(args).map(|x| Node::Num(IntegerNode::new(x)))
        })
    }

    fn run_prim_op_nf(
        &mut self,
        prim_op: PrimOpKind,
        arg_addrs: Vec<Addr>,
    ) -> Result<Node, String> {
        match prim_op {
            PrimOpKind::Neg => self.impl_prim_all_num_args_ret_num(arg_addrs, |[x]| Ok(-x)),
            PrimOpKind::Add => self.impl_prim_all_num_args_ret_num(arg_addrs, |[x, y]| Ok(x + y)),
            PrimOpKind::Sub => self.impl_prim_all_num_args_ret_num(arg_addrs, |[x, y]| Ok(x - y)),
            PrimOpKind::Mul => self.impl_prim_all_num_args_ret_num(arg_addrs, |[x, y]| Ok(x * y)),
            PrimOpKind::Div => self.impl_prim_all_num_args_ret_num(arg_addrs, |[x, y]| {
                if y == 0 {
                    Err("divide by zero".to_string())
                } else {
                    Ok(x / y)
                }
            }),
            PrimOpKind::Eq => self.impl_prim_all_num_args(arg_addrs, |[x, y]| {
                let tag = if x == y { TRUE_TAG } else { FALSE_TAG };
                Ok(Node::Data(DataNode::new(tag, vec![])))
            }),
            PrimOpKind::Lt => self.impl_prim_all_num_args(arg_addrs, |[x, y]| {
                let tag = if x < y { TRUE_TAG } else { FALSE_TAG };
                Ok(Node::Data(DataNode::new(tag, vec![])))
            }),
            PrimOpKind::Gt => self.impl_prim_all_num_args(arg_addrs, |[x, y]| {
                let tag = if x > y { TRUE_TAG } else { FALSE_TAG };
                Ok(Node::Data(DataNode::new(tag, vec![])))
            }),
            _ => panic!("BUG: run_prim_op_nf doesn't handle constructor or if-then-else"),
        }
    }

    fn push_stack_frame(&mut self) {
        let mut stack_to_save = Stack::new();
        mem::swap(&mut stack_to_save, &mut self.stack);
        self.dump.push(stack_to_save);
    }

    fn pop_stack_frame(&mut self) {
        let stack = self.dump.pop().unwrap();
        self.stack = stack;
    }

    fn handle_super_comb_node(
        &mut self,
        _node_addr: Addr,
        n: &SuperCombinatorNode,
    ) -> Result<(), String> {
        let super_comb_node_addr = self.stack.pop().unwrap(); // The ptr to the super combinator node
        let num_args = n.0.arguments.len();
        let ap_node_addrs = self.stack.pop_n_releaxed(num_args);
        if num_args != ap_node_addrs.len() {
            Err(format!(
                "super combinator {:?} expected {:} args, got {:}",
                n,
                num_args,
                ap_node_addrs.len()
            ))?
        }
        let node_addr_to_update = if num_args == 0 {
            super_comb_node_addr
        } else {
            *ap_node_addrs.last().unwrap()
        };

        let env_args = ap_node_addrs
            .iter()
            .zip(n.0.arguments.clone())
            .map(|(addr, name)| {
                let r_addr = self.must_get_application_node_r_at(*addr);
                (name, r_addr)
            })
            .fold(Assoc::new(), |mut a, (name, addr)| {
                a.insert(name, addr);
                a
            });
        let env = Assoc::combine(self.globals.clone(), env_args);
        let addr = self.instantiate(&env, &n.0.body, Some(node_addr_to_update))?;

        assert_eq!(addr, node_addr_to_update);
        self.stack.push(addr);

        Ok(())
    }

    fn must_get_application_node_r_at(&self, addr: Addr) -> Addr {
        self.follow_indirect(match self.heap.access(addr).unwrap().borrow().deref() {
            Node::Ap(ap_node) => ap_node.r,
            node => panic!("BUG: expected Ap node, got {:?}", node),
        })
    }

    fn alloc_node(&mut self, n: Node) -> Addr {
        self.heap.alloc(Node::new_in_rc_refcell(n))
    }

    fn replace_node_at(&mut self, a: Addr, n: Node) {
        *self.heap.access_mut(a).unwrap() = Node::new_in_rc_refcell(n);
    }

    fn replace_or_alloc_node_at(&mut self, replace_at: Option<Addr>, node: Node) -> Addr {
        match (replace_at, node) {
            (Some(addr), node) => {
                self.replace_node_at(addr, node);
                addr
            }
            (None, Node::Indirect(addr)) => addr,
            (None, n) => self.alloc_node(n),
        }
    }

    fn instantiate(
        &mut self,
        env: &Assoc<ast::Name, Addr>,
        expr: &ast::Expr<ast::Name>,
        // To handle recursive let where we need to know the binder's addr before instatiating the respective expression.
        replace_at: Option<Addr>,
    ) -> Result<Addr, String> {
        match expr {
            ast::Expr::Num(n) => {
                Ok(self.replace_or_alloc_node_at(replace_at, Node::Num(IntegerNode::new(n.0))))
            }
            ast::Expr::Ap(ap) => {
                let l_addr = self.instantiate(env, &ap.l, None)?;
                let r_addr = self.instantiate(env, &ap.r, None)?;
                Ok(self.replace_or_alloc_node_at(
                    replace_at,
                    Node::Ap(ApplicationNode::new(l_addr, r_addr)),
                ))
            }
            ast::Expr::Var(v) => env
                .lookup(v)
                .copied()
                .ok_or(format!("variable {:?} not found", v))
                .map(|a| self.replace_or_alloc_node_at(replace_at, Node::Indirect(a))),
            ast::Expr::Let(l) => {
                let preallocated_binders = l.is_recursive.then(|| {
                    l.definitions
                        .iter()
                        .map(|b| (b.binder.clone(), self.alloc_node(Node::Dummy)))
                        .fold(Assoc::new(), |mut a, (k, v)| {
                            a.insert(k, v);
                            a
                        })
                });
                let rec_env = preallocated_binders
                    .clone()
                    .map(|p| Assoc::combine(env.clone(), p));
                let env = rec_env.as_ref().unwrap_or(env);
                let preallocated_binders = preallocated_binders.unwrap_or(Assoc::new());

                let binders = l
                    .definitions
                    .iter()
                    .map(|b| {
                        let addr = self.instantiate(
                            env,
                            &b.body,
                            preallocated_binders.lookup(&b.binder).copied(),
                        )?;
                        Ok((b.binder.clone(), addr))
                    })
                    .collect::<Result<Vec<(ast::Name, Addr)>, String>>()?
                    .into_iter()
                    .fold(Assoc::new(), |mut a, (k, v)| {
                        a.insert(k, v);
                        a
                    });
                let env = Assoc::combine(env.clone(), binders);
                let env = &env;

                self.instantiate(env, &l.body, replace_at)
            }
            ast::Expr::Constr(c) => Ok(self.replace_or_alloc_node_at(
                replace_at,
                Node::Prim(PrimNode::new(PrimOp::Constr(ConstrPrimOp::new(
                    c.tag.0,
                    c.arity.0 as usize,
                )))),
            )),
            // FIXME
            e => panic!("BUG: cannot instantiate this variant yet: {:?}", e),
        }
    }

    // Current root redex cannot be further reduce?
    fn is_fully_reduce(&self) -> bool {
        match self.stack.len() {
            0 => panic!("BUG: current stack is empty"),
            1 => self.peak_node().borrow().is_data_node(),
            _ => false,
        }
    }

    pub fn peak_node(&self) -> Rc<RefCell<Node>> {
        let addr = *self.stack.peak().unwrap();
        let node = self.heap.access(addr).unwrap();
        node.clone()
    }
}
