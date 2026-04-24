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

    pub fn push_vec(&mut self, v: Vec<T>) {
        v.into_iter().for_each(|x| self.push(x));
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
        MatchPair,
        MatchList,
        Abort,
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
            PrimOpKind::MatchPair => Some("_prim_match_pair"),
            PrimOpKind::MatchList => Some("_prim_match_list"),
            PrimOpKind::Abort => Some("_prim_abort"),
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
    MatchPair,
    MatchList,
    Abort,
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
            PrimOpKind::MatchPair => Some(PrimOp::MatchPair),
            PrimOpKind::MatchList => Some(PrimOp::MatchList),
            PrimOpKind::Abort => Some(PrimOp::Abort),
            PrimOpKind::Constr => None,
        }
    }

    fn _get_kind(&self) -> PrimOpKind {
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
            PrimOp::MatchPair => PrimOpKind::MatchPair,
            PrimOp::MatchList => PrimOpKind::MatchList,
            PrimOp::Abort => PrimOpKind::Abort,
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
            PrimOp::MatchPair => 2,
            PrimOp::MatchList => 3,
            PrimOp::Abort => 0,
            PrimOp::Constr(c) => c.arity as usize,
        }
    }
}

#[derive(Debug, Clone)]
enum PrimOpResult {
    NeedFurtherEvaluate(Addr),
    Done(Node),
}

#[derive(Debug, Clone)]
enum PrimOpArgAddr {
    DataNode(Addr),
    NotDataNode(Addr),
}

impl PrimOpArgAddr {
    pub fn get_addr(&self) -> Addr {
        *match self {
            PrimOpArgAddr::DataNode(addr) => addr,
            PrimOpArgAddr::NotDataNode(addr) => addr,
        }
    }

    pub fn is_whnf(&self) -> bool {
        match self {
            PrimOpArgAddr::DataNode(_) => true,
            PrimOpArgAddr::NotDataNode(_) => false,
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

pub const PAIR_TAG: u64 = 0;

pub const NIL_TAG: u64 = 0;
pub const CONS_TAG: u64 = 1;

fn extended_prelude() -> Vec<ast::SuperCombinator<ast::Name>> {
    vec![
        must_lex_and_parse_sc(format!("neg = {}", PrimOpKind::Neg.to_name().unwrap())),
        must_lex_and_parse_sc(format!("false = Pack{{{},0}}", FALSE_TAG)),
        must_lex_and_parse_sc(format!("true = Pack{{{},0}}", TRUE_TAG)),
        must_lex_and_parse_sc(format!(
            "if = {}",
            PrimOpKind::IfThenElse.to_name().unwrap()
        )),
        must_lex_and_parse_sc(format!("{} x y = if x y false", PRIM_BOOLEAN_AND_NAME)),
        must_lex_and_parse_sc(format!("{} x y = if x true y", PRIM_BOOLEAN_OR_NAME)),
        must_lex_and_parse_sc("not x = if x false true"),
        must_lex_and_parse_sc("xor x y = if x (not y) y"),
        must_lex_and_parse_sc(format!("{} x y = not ({} x y)", PRIM_NE_NAME, PRIM_EQ_NAME)),
        must_lex_and_parse_sc(format!("{} x y = (x < y) || (x == y) ", PRIM_LE_NAME)),
        must_lex_and_parse_sc(format!("{} x y = (x > y) || (x == y) ", PRIM_GE_NAME)),
        must_lex_and_parse_sc(format!("mkPair a b = Pack{{{}, 2}} a b", PAIR_TAG)),
        must_lex_and_parse_sc(format!(
            "casePair p f = {} p f",
            PrimOpKind::MatchPair.to_name().unwrap()
        )),
        must_lex_and_parse_sc("fst p = casePair p k"),
        must_lex_and_parse_sc("snd p = casePair p k1"),
        must_lex_and_parse_sc(format!("cons = Pack{{{}, 2}}", CONS_TAG)),
        must_lex_and_parse_sc(format!("nil = Pack{{{}, 0}}", NIL_TAG)),
        must_lex_and_parse_sc(format!(
            "caseList l onNil onCons = {} l onNil onCons",
            PrimOpKind::MatchList.to_name().unwrap()
        )),
        must_lex_and_parse_sc("length l = caseList l lengthOnNil lengthOnCons"),
        must_lex_and_parse_sc("lengthOnNil = 0"),
        must_lex_and_parse_sc("lengthOnCons x xs = 1 + length xs"),
        must_lex_and_parse_sc("head l = caseList l abort K"),
        must_lex_and_parse_sc("tail l = caseList l abort K1"),
        must_lex_and_parse_sc(format!("panic = {}", PrimOpKind::Abort.to_name().unwrap())),
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

    fn impl_prim_all_num_args<const N: usize, F>(
        &mut self,
        arg_addrs: Vec<PrimOpArgAddr>,
        f: F,
    ) -> Result<PrimOpResult, String>
    where
        F: Fn([i64; N]) -> Result<Node, String>,
    {
        let (to_be_evaluated, evaluated): (Vec<Addr>, Vec<Addr>) =
            arg_addrs.into_iter().partition_map(|addr| match addr {
                PrimOpArgAddr::DataNode(addr) => Either::Right(addr),
                PrimOpArgAddr::NotDataNode(addr) => Either::Left(addr),
            });

        if let Some(addr) = to_be_evaluated.first() {
            return Ok(PrimOpResult::NeedFurtherEvaluate(*addr));
        }

        let num_args: Vec<i64> = evaluated
            .into_iter()
            .map(
                |addr| match self.heap.access(addr).unwrap().borrow().deref() {
                    Node::Num(n) => Ok(n.0),
                    n => Err(format!("expected integer node at {:?}, got {:?}", addr, n)),
                },
            )
            .try_collect()?;
        let num_args_arr: [i64; N] = num_args
            .try_into()
            .map_err(|v: Vec<i64>| format!("expected {} args, got {}", N, v.len()))?;

        let node = f(num_args_arr)?;
        Ok(PrimOpResult::Done(node))
    }

    fn impl_prim_all_num_args_ret_num<const N: usize, F>(
        &mut self,
        arg_addrs: Vec<PrimOpArgAddr>,
        f: F,
    ) -> Result<PrimOpResult, String>
    where
        F: Fn([i64; N]) -> Result<i64, String>,
    {
        self.impl_prim_all_num_args(arg_addrs, |args| {
            f(args).map(|x| Node::Num(IntegerNode::new(x)))
        })
    }

    fn impl_prim_all_num_args_ret_bool<const N: usize, F>(
        &mut self,
        arg_addrs: Vec<PrimOpArgAddr>,
        f: F,
    ) -> Result<PrimOpResult, String>
    where
        F: Fn([i64; N]) -> Result<bool, String>,
    {
        self.impl_prim_all_num_args(arg_addrs, |args| {
            f(args).map(|b| {
                let tag = if b { TRUE_TAG } else { FALSE_TAG };
                Node::Data(DataNode::new(tag, vec![]))
            })
        })
    }

    fn impl_prim_constr(
        &mut self,
        arg_addrs: Vec<PrimOpArgAddr>,
        constr_prim_op: &ConstrPrimOp,
    ) -> Result<PrimOpResult, String> {
        let arity = constr_prim_op.arity;

        let num_fields_got = arg_addrs.len();
        if num_fields_got != arity {
            return Err(format!(
                "constructor expected {} fields, got {}",
                arity, num_fields_got
            ));
        }

        let field_args = arg_addrs
            .into_iter()
            .map(|x| x.get_addr())
            .collect::<Vec<_>>();
        let node = Node::Data(DataNode::new(constr_prim_op.tag, field_args));
        Ok(PrimOpResult::Done(node))
    }

    fn impl_prim_if_then_else(
        &mut self,
        arg_addrs: Vec<PrimOpArgAddr>,
    ) -> Result<PrimOpResult, String> {
        let [pred_addr, then_branch_addr, else_branch_addr] =
            arg_addrs.try_into().map_err(|v: Vec<PrimOpArgAddr>| {
                format!("if-then-else prim op expected 3 args, got {}", v.len())
            })?;

        if !pred_addr.is_whnf() {
            return Ok(PrimOpResult::NeedFurtherEvaluate(pred_addr.get_addr()));
        }

        match self
            .heap
            .access(pred_addr.get_addr())
            .unwrap()
            .borrow()
            .deref()
        {
            Node::Num(_) => {
                // Otherwise it won't stop evaluating
                Err("predicate expression evaluated to num".to_string())
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
                Ok(PrimOpResult::Done(Node::Indirect(next_addr.get_addr())))
            }
            _ => unreachable!("BUG: pred is not in whnf"),
        }
    }

    fn impl_prim_match_pair(
        &mut self,
        arg_addrs: Vec<PrimOpArgAddr>,
    ) -> Result<PrimOpResult, String> {
        let [pair_addr, f_addr] = arg_addrs.try_into().map_err(|v: Vec<PrimOpArgAddr>| {
            format!("matchPair prim op expected 2 args, got {}", v.len())
        })?;

        if !pair_addr.is_whnf() {
            return Ok(PrimOpResult::NeedFurtherEvaluate(pair_addr.get_addr()));
        }

        let (a_addr, b_addr) = match self
            .heap
            .access(pair_addr.get_addr())
            .unwrap()
            .borrow()
            .deref()
        {
            Node::Num(_) => {
                // Otherwise it won't stop evaluating
                Err("pair expression evaluated to num, while data is expected".to_string())
            }
            Node::Data(d) => match (d.tag, d.field_addrs.len()) {
                (PAIR_TAG, 2) => {
                    let [a_addr, b_addr] = d.field_addrs.clone().try_into().unwrap();
                    Ok((a_addr, b_addr))
                }
                (tag, fields_len) => Err(format!(
                    "unrecognized pair constructor, tag: {}, fields len: {}",
                    tag, fields_len
                )),
            },
            _ => unreachable!("BUG: pair is not in whnf"),
        }?;

        let node = Node::Ap(ApplicationNode::new(f_addr.get_addr(), a_addr));
        let l_addr = self.alloc_node(node);
        let node = Node::Ap(ApplicationNode::new(l_addr, b_addr));
        Ok(PrimOpResult::Done(node))
    }

    fn impl_prim_match_list(
        &mut self,
        arg_addrs: Vec<PrimOpArgAddr>,
    ) -> Result<PrimOpResult, String> {
        let [list_addr, on_nil_addr, on_cons_addr]: [PrimOpArgAddr; 3] =
            arg_addrs.try_into().map_err(|v: Vec<PrimOpArgAddr>| {
                format!("matchList prim op expected 3 args, got {}", v.len())
            })?;

        if !list_addr.is_whnf() {
            return Ok(PrimOpResult::NeedFurtherEvaluate(list_addr.get_addr()));
        }

        let next = match self
            .heap
            .access(list_addr.get_addr())
            .unwrap()
            .borrow()
            .deref()
        {
            Node::Num(_) => {
                // Otherwise it won't stop evaluating
                Err("list expression evaluated to num, while data is expected".to_string())
            }
            Node::Data(d) => match (d.tag, d.field_addrs.len()) {
                (NIL_TAG, 0) => Ok(Either::Left(())),
                (CONS_TAG, 2) => {
                    let [head_addr, tail_addr] = d.field_addrs.clone().try_into().unwrap();
                    Ok(Either::Right((head_addr, tail_addr)))
                }
                (tag, fields_len) => Err(format!(
                    "unrecognized list constructor, tag: {}, fields len: {}",
                    tag, fields_len
                )),
            },
            _ => unreachable!("BUG: pair is not in whnf"),
        }?;

        Ok(PrimOpResult::Done(match next {
            Either::Left(_) => Node::Indirect(on_nil_addr.get_addr()),
            Either::Right((head_addr, tail_addr)) => {
                let node = Node::Ap(ApplicationNode::new(on_cons_addr.get_addr(), head_addr));
                let l_addr = self.alloc_node(node);
                let node = Node::Ap(ApplicationNode::new(l_addr, tail_addr));
                node
            }
        }))
    }

    fn handle_prim_node(&mut self, node_addr: Addr, prim_node: &PrimNode) -> Result<(), String> {
        assert_eq!(self.stack.pop(), Some(node_addr));
        let arity = prim_node.0.get_arity();
        let ap_node_addrs = self.stack.pop_n_releaxed(arity);
        let num_popped = ap_node_addrs.len();

        if num_popped != arity {
            Err(format!(
                "prim op {:?} expected {} args, got {}",
                prim_node.0, arity, num_popped
            ))?
        }

        let node_addr_to_override = if arity == 0 {
            node_addr
        } else {
            *ap_node_addrs.last().unwrap()
        };

        let arg_addrs = ap_node_addrs
            .iter()
            .map(|addr| {
                let arg_addr = self.must_get_application_node_r_at(*addr);
                let node = self.heap.access(arg_addr).unwrap().borrow();
                if node.is_data_node() {
                    PrimOpArgAddr::DataNode(arg_addr)
                } else {
                    PrimOpArgAddr::NotDataNode(arg_addr)
                }
            })
            .collect::<Vec<_>>(); // Right: NF

        let res = match &prim_node.0 {
            PrimOp::Neg => self.impl_prim_all_num_args_ret_num(arg_addrs, |[x]| Ok(-x)),
            PrimOp::Add => self.impl_prim_all_num_args_ret_num(arg_addrs, |[x, y]| Ok(x + y)),
            PrimOp::Sub => self.impl_prim_all_num_args_ret_num(arg_addrs, |[x, y]| Ok(x - y)),
            PrimOp::Mul => self.impl_prim_all_num_args_ret_num(arg_addrs, |[x, y]| Ok(x * y)),
            PrimOp::Div => self.impl_prim_all_num_args_ret_num(arg_addrs, |[x, y]| {
                if y == 0 {
                    Err("divide by zero".to_string())
                } else {
                    Ok(x / y)
                }
            }),
            PrimOp::Eq => self.impl_prim_all_num_args_ret_bool(arg_addrs, |[x, y]| Ok(x == y)),
            PrimOp::Lt => self.impl_prim_all_num_args_ret_bool(arg_addrs, |[x, y]| Ok(x < y)),
            PrimOp::Gt => self.impl_prim_all_num_args_ret_bool(arg_addrs, |[x, y]| Ok(x > y)),
            PrimOp::Constr(constr_prim_op) => self.impl_prim_constr(arg_addrs, constr_prim_op),
            PrimOp::MatchPair => self.impl_prim_match_pair(arg_addrs),
            PrimOp::MatchList => self.impl_prim_match_list(arg_addrs),
            PrimOp::IfThenElse => self.impl_prim_if_then_else(arg_addrs),
            PrimOp::Abort => Err("user code called abort".to_string()),
        }?;

        match res {
            PrimOpResult::NeedFurtherEvaluate(eval_addr) => {
                self.stack
                    .push_vec(ap_node_addrs.into_iter().rev().collect());
                self.stack.push(node_addr);
                self.push_stack_frame();
                self.stack.push(eval_addr);
            }
            PrimOpResult::Done(node) => {
                self.replace_node_at(node_addr_to_override, node);
                self.stack.push(node_addr_to_override);
            }
        };

        Ok(())
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
