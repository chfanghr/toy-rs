use std::{cmp::max, mem::replace};

use crate::parser::{
    ast, prelude, PRIM_ADD_NAME, PRIM_DIV_NAME, PRIM_EQ_NAME, PRIM_GT_NAME, PRIM_LT_NAME,
    PRIM_MUL_NAME, PRIM_SUB_NAME,
};

use super::{assoc::Assoc, heap::Addr, heap::Heap, prelude::extended_prelude, stack::Stack};
use anyhow::{anyhow, Result};
use log::trace;

#[derive(Debug, Clone)]
pub(super) enum Node {
    Ap(ApplicationNode),
    SuperComb(SuperCombinatorNode),
    Num(IntegerNode),
    Prim(PrimNode),
    Data(DataNode),
    Indirect(Addr),
    Dummy,
}

impl Node {
    pub(super) fn is_data_node(&self) -> bool {
        match self {
            Node::Num(_) => true,
            Node::Data(_) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ApplicationNode {
    l: Addr,
    r: Addr,
}

impl ApplicationNode {
    pub(super) fn new(l: Addr, r: Addr) -> Self {
        Self { l, r }
    }

    pub(super) fn l_addr(&self) -> Addr {
        self.l
    }

    pub(super) fn r_addr(&self) -> Addr {
        self.r
    }
}

#[derive(Debug, Clone)]
pub(super) struct SuperCombinatorNode(ast::SuperCombinator<ast::Name>);

impl SuperCombinatorNode {
    pub(super) fn new(sc: ast::SuperCombinator<ast::Name>) -> SuperCombinatorNode {
        SuperCombinatorNode(sc)
    }

    pub(super) fn inner(&self) -> &ast::SuperCombinator<ast::Name> {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub(super) struct IntegerNode(i64);

impl IntegerNode {
    pub(super) fn new(i: i64) -> IntegerNode {
        IntegerNode(i)
    }

    pub(super) fn val(&self) -> i64 {
        self.0
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
        MatchPair,
        MatchList,
        Abort,
        Stop,
        Print,
        Seq,
        Constr
    }
}

impl PrimOpKind {
    pub(super) fn to_name(&self) -> Option<&'static str> {
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
            PrimOpKind::MatchPair => Some("_prim_match_pair"),
            PrimOpKind::MatchList => Some("_prim_match_list"),
            PrimOpKind::Abort => Some("_prim_abort"),
            PrimOpKind::Stop => Some("_prim_stop"),
            PrimOpKind::Print => Some("_prim_print"),
            PrimOpKind::Seq => Some("_prim_seq"),
            PrimOpKind::Constr => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum PrimOp {
    Neg,
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Lt,
    Gt,
    IfThenElse,
    MatchPair,
    MatchList,
    Abort,
    Stop,
    Print,
    Seq,
    Constr(ConstrPrimOp),
}

impl PrimOp {
    pub(super) fn new_from_kind(k: PrimOpKind) -> Option<Self> {
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
            PrimOpKind::MatchPair => Some(PrimOp::MatchPair),
            PrimOpKind::MatchList => Some(PrimOp::MatchList),
            PrimOpKind::Abort => Some(PrimOp::Abort),
            PrimOpKind::Stop => Some(PrimOp::Stop),
            PrimOpKind::Print => Some(PrimOp::Print),
            PrimOpKind::Seq => Some(PrimOp::Seq),
            PrimOpKind::Constr => None,
        }
    }

    pub(super) fn get_arity(&self) -> usize {
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
            PrimOp::MatchPair => 2,
            PrimOp::MatchList => 3,
            PrimOp::Abort => 0,
            PrimOp::Stop => 0,
            PrimOp::Print => 2,
            PrimOp::Seq => 2,
            PrimOp::Constr(c) => c.arity as usize,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConstrPrimOp {
    tag: u64,
    arity: usize,
}

impl ConstrPrimOp {
    pub(super) fn new(tag: u64, arity: usize) -> Self {
        Self { tag, arity }
    }

    pub(super) fn tag(&self) -> u64 {
        self.tag
    }

    pub(super) fn arity(&self) -> usize {
        self.arity
    }
}

#[derive(Debug, Clone)]
pub(super) struct PrimNode(PrimOp);

impl PrimNode {
    pub(super) fn new_from_kind(k: PrimOpKind) -> Option<Self> {
        PrimOp::new_from_kind(k).map(Self)
    }

    pub(super) fn new(o: PrimOp) -> Self {
        Self(o)
    }

    pub(super) fn prim_op(&self) -> &PrimOp {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub(super) struct DataNode {
    tag: u64,
    field_addrs: Vec<Addr>,
}

impl DataNode {
    pub(super) fn new(tag: u64, field_addrs: Vec<Addr>) -> Self {
        Self { tag, field_addrs }
    }

    pub(super) fn tag(&self) -> u64 {
        self.tag
    }

    pub(super) fn field_addrs(&self) -> &[Addr] {
        &self.field_addrs
    }
}

#[derive(Debug, Clone)]
pub struct Stats {
    steps: usize,
    peak_heap_size: usize,
}

impl Stats {
    fn new() -> Self {
        Self {
            steps: 0,
            peak_heap_size: 0,
        }
    }

    fn incr_steps(&mut self) {
        self.steps += 1;
    }

    fn update_heap_size(&mut self, s: usize) {
        self.peak_heap_size = max(self.peak_heap_size, s)
    }

    fn reset(&mut self) {
        _ = replace(self, Self::new());
    }

    pub fn steps(&self) -> usize {
        self.steps
    }

    pub fn peak_heap_size(&self) -> usize {
        self.peak_heap_size
    }
}

#[derive(Debug, Clone)]
pub(super) enum PrimOpResult {
    NeedFurtherEvaluate(Addr),
    Done(Node),
    Stop,
}

#[derive(Debug, Clone)]
pub(super) enum PrimOpArgAddr {
    DataOrNum(Addr),
    Other(Addr),
}

impl PrimOpArgAddr {
    pub(super) fn is_whnf(&self) -> bool {
        match self {
            PrimOpArgAddr::DataOrNum(_) => true,
            PrimOpArgAddr::Other(_) => false,
        }
    }

    pub(super) fn addr(&self) -> Addr {
        *match self {
            PrimOpArgAddr::DataOrNum(addr) => addr,
            PrimOpArgAddr::Other(addr) => addr,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Machine {
    stack: Stack<Addr>,
    current_stack_bottom: usize,
    dump: Stack<usize>, // (current height, bottom)
    heap: Heap<Node>,
    globals: Assoc<ast::Name, Addr>,
    stats: Stats,
    output: Vec<i64>,
}

fn build_initial_heap(
    scs: Vec<ast::SuperCombinator<ast::Name>>,
) -> (Heap<Node>, Assoc<ast::Name, Addr>) {
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
                let addr = heap.alloc(node);
                globals.insert(name, addr);
                (heap, globals)
            },
        )
}

impl Machine {
    pub fn new(p: ast::Program<ast::Name>) -> Self {
        let (heap, globals) = build_initial_heap(p.0);
        let stack = Stack::new();
        let current_stack_bottom = 0;
        let dump = Stack::new();
        let stats = Stats::new();
        let output = Vec::new();
        Machine {
            stack,
            current_stack_bottom,
            dump,
            heap,
            globals,
            stats,
            output,
        }
    }

    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    pub fn output(&self) -> &[i64] {
        &self.output
    }

    pub(super) fn alloc_node(&mut self, n: Node) -> Addr {
        self.heap.alloc(n)
    }

    pub(super) fn replace_or_alloc_node_at(
        &mut self,
        replace_at: Option<Addr>,
        node: Node,
    ) -> Addr {
        match (replace_at, node) {
            (Some(addr), node) => {
                self.replace_node_at(addr, node);
                addr
            }
            (None, Node::Indirect(addr)) => addr,
            (None, n) => self.alloc_node(n),
        }
    }

    pub(super) fn follow_indirect(&self, a: Addr) -> Addr {
        match self.heap.access(a).unwrap() {
            Node::Indirect(a) => self.follow_indirect(*a),
            _ => a,
        }
    }

    pub(super) fn replace_node_at(&mut self, a: Addr, n: Node) {
        *self.heap.access_mut(a).unwrap() = n;
    }

    pub(super) fn must_get_node(&self, a: Addr) -> &Node {
        let addr = self.follow_indirect(a);
        self.heap.access(addr).unwrap()
    }

    pub(super) fn push_output(&mut self, i: i64) {
        self.output.push(i);
    }

    fn do_admin(&mut self) {
        self.stats.incr_steps();
        self.stats.update_heap_size(self.heap.size());
    }

    pub(super) fn peak_node(&self) -> (Addr, &Node) {
        let addr = *self.stack.peak().unwrap();
        (addr, self.must_get_node(addr))
    }

    pub(super) fn must_get_application_node_r_at(&self, addr: Addr) -> Addr {
        self.follow_indirect(match self.must_get_node(addr) {
            Node::Ap(ap_node) => ap_node.r,
            node => panic!("BUG: expected Ap node, got {:?}", node),
        })
    }

    pub(super) fn clear_stack(&mut self) {
        self.stack.set_height(self.current_stack_bottom);
    }

    pub(super) fn push_stack_frame(&mut self) {
        let height = self.stack_height();
        trace!("push_stack_frame height={}", height);
        self.dump.push(height);
        self.current_stack_bottom = height;
    }

    fn pop_stack_frame(&mut self) {
        let height = self.dump.pop_cloned().unwrap();
        let bottom = self.dump.peak_cloned().unwrap_or(0);
        trace!("pop_stack_frame height={} bottom={}", height, bottom);
        self.stack.set_height(height);
        self.current_stack_bottom = bottom;
    }

    pub(super) fn assert_pop_stack(&mut self, node_addr: Addr) {
        assert!(self.stack.height() > self.current_stack_bottom);
        assert_eq!(self.stack.pop_cloned(), Some(node_addr))
    }

    pub(super) fn push_stack(&mut self, addr: Addr) {
        self.stack.push(addr);
    }

    pub(super) fn pop_stack_n(&mut self, n: usize) -> Vec<Addr> {
        assert!(self.stack.height() - self.current_stack_bottom >= n);
        self.stack.pop_n_cloned(n)
    }

    pub(super) fn stack_height(&self) -> usize {
        self.stack.height()
    }

    pub(super) fn set_stack_height(&mut self, h: usize) {
        self.stack.set_height(h);
    }

    pub(super) fn globals(&self) -> &Assoc<ast::Name, Addr> {
        &self.globals
    }

    pub fn eval(&mut self, entry_point: &ast::Name) -> Result<()> {
        let entry_point_addr = *self
            .globals
            .lookup(entry_point)
            .ok_or(anyhow!("entry point '{:?}' not found", entry_point))?;

        self.stack.reset();
        self.current_stack_bottom = 0;

        self.dump.reset();

        self.output.clear();

        self.stats.reset();

        self.stack.push(entry_point_addr);

        loop {
            trace!("machine state: {:?}\n", self);

            match self.stack.height() - self.current_stack_bottom {
                // there is no redex, we are done
                0 => break,
                // current redex in WHNF, we are done for now
                1 if self.peak_node().1.is_data_node() => {
                    if self.dump.is_empty() {
                        break;
                    } else {
                        self.pop_stack_frame();
                    }
                }
                _ => {
                    self.eval_step()?;
                    self.do_admin();
                }
            }
        }

        Ok(())
    }

    fn eval_step(&mut self) -> Result<()> {
        let (addr, node) = self.peak_node();
        self.dispatch_node(addr, node.clone())
    }

    pub fn inspect_global(&self, n: &ast::Name) -> String {
        let addr = *self
            .globals
            .lookup(n)
            .ok_or(anyhow!("global '{:?}' not found", n))
            .unwrap();
        let node = self.must_get_node(addr);
        format!("{:?}", node)
    }
}
