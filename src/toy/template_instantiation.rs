use std::{
    cell::RefCell,
    collections::{BTreeMap, LinkedList},
    mem,
    ops::Deref,
    rc::Rc,
};

use itertools::{Either, Itertools};

use crate::parser::{ast, prelude};

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
        Constr
    }
}

impl PrimOpKind {
    fn to_name(&self) -> Option<&'static str> {
        match self {
            PrimOpKind::Neg => Some("_prim_neg"),
            PrimOpKind::Add => Some("_prim_add"),
            PrimOpKind::Sub => Some("_prim_sub"),
            PrimOpKind::Mul => Some("_prim_mul"),
            PrimOpKind::Div => Some("_prim_div"),
            PrimOpKind::Constr => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConstrPrimOP {
    pub tag: u64,
    pub arity: u64,
}

impl ConstrPrimOP {
    pub fn new(tag: u64, arity: u64) -> Self {
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
    Constr(ConstrPrimOP),
}

impl PrimOp {
    fn new_from_kind(k: PrimOpKind) -> Option<Self> {
        match k {
            PrimOpKind::Neg => Some(PrimOp::Neg),
            PrimOpKind::Add => Some(PrimOp::Add),
            PrimOpKind::Sub => Some(PrimOp::Sub),
            PrimOpKind::Mul => Some(PrimOp::Mul),
            PrimOpKind::Div => Some(PrimOp::Div),
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
    pub tag: i64,
    pub field_addrs: Vec<Addr>,
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
        .chain(prelude().into_iter().chain(scs).map(|sc| {
            (
                sc.name.clone(),
                Node::SuperComb(SuperCombinatorNode::new(sc)),
            )
        }))
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
        match &prim.0.get_kind() {
            PrimOpKind::Constr => self.handle_prim_node_cosntr(node_addr, prim),
            _ => self.handle_prim_node_nf(node_addr, prim),
        }
    }

    fn handle_prim_node_cosntr(&mut self, _node_addr: Addr, prim: &PrimNode) -> Result<(), String> {
        todo!()
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
            .map(
                |addr| match self.heap.access(addr).unwrap().borrow().deref() {
                    Node::Ap(ap) => self.follow_indirect(ap.r),
                    node => panic!("BUG: expected Ap node, got {:?}", node),
                },
            )
            .partition_map(|arg_addr| {
                let node = self.heap.access(arg_addr).unwrap().borrow();
                if node.is_data_node() {
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
        F: Fn([i64; N]) -> Result<i64, String>,
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
        let res = f(args_arr).map_err(|e| format!("error while executing the prim op: {}", e))?;
        Ok(Node::Num(IntegerNode(res)))
    }

    fn run_prim_op_nf(
        &mut self,
        prim_op: PrimOpKind,
        arg_addrs: Vec<Addr>,
    ) -> Result<Node, String> {
        match prim_op {
            PrimOpKind::Neg => self.impl_prim_all_num_args(arg_addrs, |[x]| Ok(-x)),
            PrimOpKind::Add => self.impl_prim_all_num_args(arg_addrs, |[x, y]| Ok(x + y)),
            PrimOpKind::Sub => self.impl_prim_all_num_args(arg_addrs, |[x, y]| Ok(x - y)),
            PrimOpKind::Mul => self.impl_prim_all_num_args(arg_addrs, |[x, y]| Ok(x * y)),
            PrimOpKind::Div => self.impl_prim_all_num_args(arg_addrs, |[x, y]| {
                if y == 0 {
                    Err("divide by zero".to_string())
                } else {
                    Ok(x / y)
                }
            }),
            _ => panic!("BUG: run_prim_op_nf doesn't handle constructor"),
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
            .map(
                |(addr, name)| match self.heap.access(*addr).unwrap().borrow().deref() {
                    Node::Ap(ap_node) => (name, ap_node.r),
                    node => panic!("BUG: expected Ap node, got {:?}", node),
                },
            )
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
                Node::Prim(PrimNode::new(PrimOp::Constr(ConstrPrimOP::new(
                    c.tag.0, c.arity.0,
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
