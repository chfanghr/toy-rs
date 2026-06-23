use std::{
    collections::{HashMap, VecDeque},
    iter, mem,
    ops::Deref,
};

use anyhow::{Context, Result, anyhow, bail};
use intmap::IntMap;
use itertools::{
    Either::{Left, Right},
    Itertools,
};
use pretty::{DocAllocator, DocBuilder};

use crate::{
    g_machine::types::{Code, CompiledProgram, Instruction},
    parser::ast,
    utils::{
        heap_v2::{Addr, Heap},
        stack::Stack,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Node {
    Num(i64),
    Ap(Addr, Addr),
    Global(usize, Code),
    Indirect(Addr),
    Constr(u64, Vec<Addr>),
}

#[derive(Debug)]
enum WHNFData {
    Num(i64),
    Constr(u64, Vec<Addr>),
}

impl WHNFData {
    fn from_node(node: Node) -> Option<WHNFData> {
        match node {
            Node::Constr(tag, fields) => Some(WHNFData::Constr(tag, fields)),
            Node::Num(x) => Some(WHNFData::Num(x)),
            _ => None,
        }
    }

    const TRUE_TAG: u64 = 1;
    const FALSE_TAG: u64 = 0;

    fn box_boolean(b: bool) -> WHNFData {
        match b {
            true => WHNFData::Constr(Self::TRUE_TAG, vec![]),
            false => WHNFData::Constr(Self::FALSE_TAG, vec![]),
        }
    }

    fn unbox_boolean(&self) -> Option<bool> {
        match self {
            WHNFData::Constr(tag, fields) => {
                (fields.len() == 0).then_some(())?;
                match *tag {
                    Self::TRUE_TAG => Some(true),
                    Self::FALSE_TAG => Some(false),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn expect_num(&self) -> Option<i64> {
        match self {
            WHNFData::Num(x) => Some(*x),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum GlobalEntry {
    Name(ast::Name),
    Num(i64),
    Pack(u64, usize),
}

#[derive(Debug, Clone)]
struct EvalFrame {
    instructions: VecDeque<Instruction>,
    stack: Stack<Addr>,
}

#[derive(Debug, Clone)]
struct Stats {
    steps: usize,
}

impl Stats {
    fn new() -> Self {
        Self { steps: 0 }
    }

    fn incr_steps(&mut self) {
        self.steps += 1
    }

    fn pp<'b, D, A>(&'b self, a: &'b D) -> DocBuilder<'b, D, A>
    where
        D: DocAllocator<'b, A>,
        D::Doc: Clone,
        A: Clone,
    {
        a.concat([a.text("Steps"), a.space(), a.as_string(self.steps)])
            .group()
    }
}

#[derive(Debug, Clone)]
pub struct Machine {
    current: EvalFrame,
    dump: Vec<EvalFrame>,
    heap: Heap<Node>,
    globals: HashMap<GlobalEntry, Addr>,
    stats: Stats,
}

impl Machine {
    pub fn new(p: CompiledProgram, entry_point: ast::Name) -> Self {
        let current = EvalFrame {
            instructions: [Instruction::PushGlobal(entry_point), Instruction::Eval].into(),
            stack: Stack::new(),
        };

        let (heap, globals) = Self::build_initial_heap(p);

        Self {
            current,
            dump: Vec::new(),
            heap,
            globals,
            stats: Stats::new(),
        }
    }

    fn build_initial_heap(p: CompiledProgram) -> (Heap<Node>, HashMap<GlobalEntry, Addr>) {
        let mut heap = Heap::new();
        let mut globals = HashMap::new();

        for (name, (n_args, code)) in p.0.into_iter() {
            let addr = heap.alloc(Node::Global(n_args, code));
            globals.insert(GlobalEntry::Name(name), addr);
        }

        (heap, globals)
    }

    // Orchestration
    pub fn run(&mut self) -> Result<()> {
        while !self.done() {
            self.step()?;
        }

        Ok(())
    }

    fn done(&self) -> bool {
        self.current.instructions.is_empty()
    }

    fn step(&mut self) -> Result<()> {
        self.dispatch(self.current.instructions.front().unwrap().clone())?;
        self.do_admin()?;
        Ok(())
    }

    fn do_admin(&mut self) -> Result<()> {
        self.stats.incr_steps();
        Ok(())
    }

    #[stacksafe::stacksafe]
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
            Instruction::Branch(then_branch, else_branch) => self
                .handle_branch(then_branch.into_inner(), else_branch.into_inner())
                .context("Branch"),
            Instruction::Add => self.handle_add().context("Add"),
            Instruction::Sub => self.handle_sub().context("Sub"),
            Instruction::Mul => self.handle_mul().context("Mul"),
            Instruction::Div => self.handle_div().context("Div"),
            Instruction::Eq => self.handle_eq().context("Eq"),
            Instruction::Ne => self.handle_ne().context("Ne"),
            Instruction::Gt => self.handle_gt().context("Gt"),
            Instruction::Ge => self.handle_ge().context("Ge"),
            Instruction::Lt => self.handle_lt().context("Lt"),
            Instruction::Le => self.handle_le().context("Le"),
            Instruction::BooleanAnd => self.handle_boolean_and().context("BooleanAnd"),
            Instruction::BooleanOr => self.handle_boolean_or().context("BooleanOr"),
            Instruction::Pack(t, n) => self.handle_pack(t, n).context("Pack"),
            Instruction::PushPack(t, n) => self.handle_push_pack(t, n).context("PushPack"),
            Instruction::CaseJump(alts) => {
                let alts = alts
                    .into_iter()
                    .map(|(tag, code)| (tag, code.into_inner()))
                    .collect();
                self.handle_case_jump(alts).context("CaseJump")
            }
            Instruction::Split(n) => self.handle_split(n).context("Split"),
            Instruction::Abort => self.handle_abort().context("Abort"),
        }
    }

    // Instructions
    // Assumption: the orchestration code doesn't change the instruction sequence(self.current.instructions)

    /* PushGlobal f:i   s d h m[Name(f):a]
    =>              i a:s d h m
     */
    fn handle_push_global(&mut self, f: ast::Name) -> Result<()> {
        self.current.instructions.pop_front();

        let addr = self.lookup_global_name(f)?;
        self.current.stack.push(addr);

        Ok(())
    }

    /* PushNum x:i   s d h           m
    => i           a:s d h:[a:Num x] m[x:a]

       PushNum x:i   s d h           m[Num x:a]
    =>           i a:s d h           m
     */
    fn handle_push_num(&mut self, n: i64) -> Result<()> {
        self.current.instructions.pop_front();

        let addr = self.alloc_num_node(n);

        self.current.stack.push(addr);

        Ok(())
    }

    /* MkAp:i l:r:s d h              m
    => i        a:s d h:[a:Ap l r]   m
     */
    fn handle_mk_ap(&mut self) -> Result<()> {
        self.current.instructions.pop_front();

        let l = self
            .current
            .stack
            .pop()
            .copied()
            .ok_or(anyhow!("lhs addr missing"))?;
        let r = self
            .current
            .stack
            .pop()
            .copied()
            .ok_or(anyhow!("rhs addr missing"))?;

        let node = Node::Ap(l, r);
        let addr = self.heap.alloc(node);

        self.current.stack.push(addr);

        Ok(())
    }

    /* Push n:i     a_0:...:a_n:s d h m
    =>        i a_n:a_0:...:a_n:s d h m
     */
    fn handle_push(&mut self, n: usize) -> Result<()> {
        self.current.instructions.pop_front();

        let addr = self
            .current
            .stack
            .peak_nth_from_top_cloned(n)
            .ok_or(anyhow!("expected at least {} operands on the stack", n + 1,))?;

        self.current.stack.push(addr);

        Ok(())
    }

    /* Pop n:i a_0:...:a_n:s d h m
    =>       i             s d h m
     */
    fn handle_pop(&mut self, n: usize) -> Result<()> {
        self.current.instructions.pop_front();

        self.pop_n_verify(n)?;

        Ok(())
    }

    /* Alloc n:i           s d h   m
    =>         i a_1...a_n:s d h_1 m where h_1=h++[a_1:Indirect Null,...,a_n:Indirect Null]
     */
    fn handle_alloc(&mut self, n: usize) -> Result<()> {
        self.current.instructions.pop_front();

        for _ in 0..n {
            let addr = self.heap.alloc(Node::Indirect(Addr::null()));
            self.current.stack.push(addr);
        }

        Ok(())
    }

    /* Slide n:i a_0:...:a_n:s d h m
    =>         i         a_0:s d h m
     */
    fn handle_slide(&mut self, n: usize) -> Result<()> {
        self.current.instructions.pop_front();

        let a_0 = *self
            .current
            .stack
            .pop()
            .ok_or(anyhow!("slide on empty stack"))?;
        self.pop_n_verify(n)?;

        self.current.stack.push(a_0);

        Ok(())
    }

    /*   Eval:i a:s         d h m
    => [Unwind] [a] [(i,s)]:d h m
     */
    fn handle_eval(&mut self) -> Result<()> {
        self.current.instructions.pop_front();

        let a = self
            .current
            .stack
            .pop_cloned()
            .ok_or(anyhow!("eval on empty stack"))?;

        let new_eval_frame = EvalFrame {
            instructions: iter::once(Instruction::Unwind).collect(),
            stack: Stack::singleton(a),
        };
        let old_eval_frame = mem::replace(&mut self.current, new_eval_frame);

        self.dump.push(old_eval_frame);

        Ok(())
    }

    /* Branch t e:i a:s d h[a:Num 1] m
    =>         t++i   s d h          m

       Branch t e:i a:s d h[a:Num 0] m
    =>         e++i   s d h          m
     */
    fn handle_branch(&mut self, t: Code, e: Code) -> Result<()> {
        self.current.instructions.pop_front();

        let a = self
            .current
            .stack
            .pop_cloned()
            .ok_or(anyhow!("predicate addr missing"))?;

        let cond = self.unbox_boolean_at(a)?;
        let code = (*(if cond { t } else { e }).0).clone();

        self.current.instructions.prepend(code);

        Ok(())
    }

    /* Update n:i a:a_1:...a_n:s d h                 m
    =>          i   a_1:...a_n:s d h[a_n:Indirect a] m
    */
    fn handle_update(&mut self, n: usize) -> Result<()> {
        self.current.instructions.pop_front();

        let a = self
            .current
            .stack
            .pop_cloned()
            .ok_or(anyhow!("src addr missing"))?;
        let a_n = self
            .current
            .stack
            .peak_nth_from_top_cloned(n)
            .ok_or(anyhow!("dest addr missing"))?;

        let node = Node::Indirect(a);
        self.replace_node_at(a_n, node);

        Ok(())
    }

    /* [Unwind] a:s [] h[a:Num x] m
    =>       [] a:s [] h          m

       [Unwind] a:s   (i_1, s_1):d h[a:Num x] m
    =>      i_1 a:s_1            d h          m

    (Do the same for constructors)

       [Unwind] a:s     d h[a:Ap a_1 a_2] m
    => [Unwind] a_1:a:s d h               m

       [Unwind] a:s   d  h[a:Indirect a_1] m
    => [Unwind] a_1:s d  h                 m

       [Unwind] a:a_1:...:a_n:s d h[a:Global n c] m
    // FIXME: This is wrong, a_1, a_2, ..., a_n all points to Ap nodes.
    //        We need to extract the lhs and put them on the stack.
    =>        c   a_1:...:a_n:s d h               m

       [Unwind] [a:a_1:...:a_k] (i, s):d h[a:Global n c] m
    =>        i  a_k:s                 d h               m where k<n

       [Unwind] [a:a_1:...:a_k] [] h[a:Global n c] m
    =>       []           [a_k] [] h               m where k<n
     */
    fn handle_unwind(&mut self) -> Result<()> {
        if let instrs_left = self.current.instructions.len()
            && instrs_left != 1
        {
            bail!(
                "expected no instruction after unwind, instructions left: {}",
                instrs_left
            )
        }

        let a = self
            .current
            .stack
            .peak_cloned()
            .ok_or(anyhow!("unwind on empty stack"))?;

        match self.must_access_node(a) {
            Node::Num(_) => self.handle_unwind_whnf(a),
            Node::Constr(_, _) => self.handle_unwind_whnf(a),
            Node::Ap(l_addr, _) => {
                self.current.stack.push(*l_addr);
            }
            Node::Indirect(a_indir) => {
                let a = *a_indir;
                self.current.stack.pop().unwrap(); // Ptr to Indirect node
                self.current.stack.push(a);
            }
            Node::Global(n_args, code) => {
                let n_args = *n_args;
                let code = code.clone();

                self.current.stack.pop().unwrap(); // Ptr to Global node

                match self.pop_n_verify(n_args) {
                    Ok(ap_ptrs) => {
                        let node_to_be_updated = ap_ptrs.last().cloned().unwrap_or(a);

                        let (arg_addrs, non_ap_addrs): (Vec<_>, Vec<_>) = ap_ptrs
                            .into_iter()
                            .partition_map(|addr| match self.must_access_node(addr) {
                                Node::Ap(_, r) => Left(*r),
                                _ => Right(addr),
                            });

                        if !non_ap_addrs.is_empty() {
                            bail!(
                                "expected all {} operands point to Ap nodes, but the following were not: {:?}",
                                n_args,
                                non_ap_addrs
                            )
                        }

                        self.current.stack.push(node_to_be_updated);
                        arg_addrs
                            .into_iter()
                            .rev()
                            .for_each(|a| self.current.stack.push(a));

                        self.current.instructions = (*code.0).clone().into();
                    }
                    Err(_) => {
                        let a_k = self.current.stack.peak_bottom_cloned().unwrap_or(a);

                        self.current.instructions.pop_front();

                        if self.try_pop_eval_frame() {
                            self.current.stack.push(a_k);
                        }
                    }
                }
            }
        };

        Ok(())
    }

    fn handle_unwind_whnf(&mut self, addr: Addr) {
        self.current.instructions.pop_front();

        if self.try_pop_eval_frame() {
            self.current.stack.push(addr);
        }
    }

    /* <Op>:i     a:s d h[a:Num o]     m
    =>      i a_res:s d h[a_res:Num x] m
     */
    #[allow(dead_code)]
    fn handle_prim_unary<F>(&mut self, f: F) -> Result<()>
    where
        F: Fn(i64) -> Result<i64>,
    {
        self.current.instructions.pop_front();

        let a = self
            .current
            .stack
            .pop_cloned()
            .ok_or(anyhow!("operand not found"))?;

        let n = self.must_access_node(a);

        let o = match n {
            Node::Num(o) => f(*o),
            _ => Err(anyhow!(
                "expected operand to be in WHNF, got {:?} at {:?}",
                n,
                a
            )),
        }?;

        let a_res = self.alloc_num_node(o);

        self.current.stack.push(a_res);

        Ok(())
    }

    /* <Op>:i a_l:a_r:s d h[a_l:Num l, a_r:Num r] m
    =>      i   a_res:s d h[a_res:Num x]          m
     */
    fn handle_prim_binary_whnf<F>(&mut self, f: F) -> Result<()>
    where
        F: Fn(WHNFData, WHNFData) -> Result<WHNFData>,
    {
        self.current.instructions.pop_front();

        let a_lhs = self
            .current
            .stack
            .pop_cloned()
            .ok_or(anyhow!("lhs missing"))?;
        let a_rhs = self
            .current
            .stack
            .pop_cloned()
            .ok_or(anyhow!("rhs missing"))?;

        let lhs = self.must_access_node(a_lhs);
        let rhs = self.must_access_node(a_rhs);

        let lhs = WHNFData::from_node(lhs.clone()).ok_or(anyhow!(
            "lhs is not in WHNF: addr {:?}, node {:?}",
            a_lhs,
            lhs
        ))?;
        let rhs = WHNFData::from_node(rhs.clone()).ok_or(anyhow!(
            "rhs is not in WHNF: addr {:?}, node {:?}",
            a_lhs,
            lhs
        ))?;

        let res = f(lhs, rhs)?;

        let a_res = match res {
            WHNFData::Num(x) => self.alloc_num_node(x),
            WHNFData::Constr(tag, fields) => self.heap.alloc(Node::Constr(tag, fields)),
        };

        self.current.stack.push(a_res);

        Ok(())
    }

    fn handle_prim_binary_whnf_numerical<F>(&mut self, f: F) -> Result<()>
    where
        F: Fn(i64, i64) -> Result<i64>,
    {
        self.handle_prim_binary_whnf(|l, r| {
            let l = l.expect_num().ok_or(anyhow!("lhs is not num"))?;
            let r = r.expect_num().ok_or(anyhow!("rhs is not num"))?;
            f(l, r).map(WHNFData::Num)
        })
    }

    fn handle_add(&mut self) -> Result<()> {
        self.handle_prim_binary_whnf_numerical(|l, r| l.checked_add(r).ok_or(anyhow!("overflow")))
    }

    fn handle_sub(&mut self) -> Result<()> {
        self.handle_prim_binary_whnf_numerical(|l, r| l.checked_sub(r).ok_or(anyhow!("overflow")))
    }

    fn handle_mul(&mut self) -> Result<()> {
        self.handle_prim_binary_whnf_numerical(|l, r| l.checked_mul(r).ok_or(anyhow!("overflow")))
    }

    fn handle_div(&mut self) -> Result<()> {
        self.handle_prim_binary_whnf_numerical(|l, r| {
            l.checked_div(r).ok_or(anyhow!("overflow/divide by zero"))
        })
    }

    fn handle_prim_binary_boolean<F>(&mut self, f: F) -> Result<()>
    where
        F: Fn(bool, bool) -> bool,
    {
        self.handle_prim_binary_whnf(|l, r| {
            let (b_l, b_r) = try {
                let l = l.unbox_boolean()?;
                let r = r.unbox_boolean()?;
                (l, r)
            }
            .ok_or(anyhow!(
                "lhs or rhs is not boxed boolean: lhs {:?}, rhs {:?}",
                l,
                r
            ))?;
            Ok(WHNFData::box_boolean(f(b_l, b_r)))
        })
    }

    // FIXME: how about short circuiting.....
    fn handle_boolean_and(&mut self) -> Result<()> {
        self.handle_prim_binary_boolean(|l, r| l && r)
    }

    fn handle_boolean_or(&mut self) -> Result<()> {
        self.handle_prim_binary_boolean(|l, r| l || r)
    }

    fn handle_prim_comp<F>(&mut self, f: F) -> Result<()>
    where
        F: Fn(i64, i64) -> bool,
    {
        self.handle_prim_binary_whnf(|l, r| {
            let (n_l, n_r) = try {
                let l = l.expect_num()?;
                let r = r.expect_num()?;
                (l, r)
            }
            .ok_or(anyhow!(
                "lhs and/or rhs were not num: lhs {:?}, rhs {:?}",
                l,
                r
            ))?;
            Ok(WHNFData::box_boolean(f(n_l, n_r)))
        })
    }

    fn handle_eq(&mut self) -> Result<()> {
        self.handle_prim_comp(|l, r| l == r)
    }

    fn handle_ne(&mut self) -> Result<()> {
        self.handle_prim_comp(|l, r| l != r)
    }

    fn handle_lt(&mut self) -> Result<()> {
        self.handle_prim_comp(|l, r| l < r)
    }

    fn handle_le(&mut self) -> Result<()> {
        self.handle_prim_comp(|l, r| l <= r)
    }

    fn handle_gt(&mut self) -> Result<()> {
        self.handle_prim_comp(|l, r| l > r)
    }

    fn handle_ge(&mut self) -> Result<()> {
        self.handle_prim_comp(|l, r| l >= r)
    }

    /* Pack t n:i a_0:...:a_n-1:s d h                              m
    =>          i             a:s d h[a: Constr t [a_0,...,a_n-1]] m
     */
    fn handle_pack(&mut self, t: u64, n: usize) -> Result<()> {
        self.current.instructions.pop_front();

        let field_addrs = self.pop_n_verify(n)?;

        let node = Node::Constr(t, field_addrs);

        let addr = self.heap.alloc(node);

        self.current.stack.push(addr);

        Ok(())
    }

    /* PushPack t n:i   s d h m
    =>              i a:s d h m[a:Global n (mkPackFn t n)]
     */
    fn handle_push_pack(&mut self, t: u64, n: usize) -> Result<()> {
        self.current.instructions.pop_front();

        let entry = GlobalEntry::Pack(t, n);
        let a = self.globals.get(&entry).cloned().unwrap_or_else(|| {
            let code = Self::mk_pack_fn(t, n);
            let node = Node::Global(n, code);
            let a = self.heap.alloc(node);
            self.globals.insert(entry, a);
            a
        });

        self.current.stack.push(a);

        Ok(())
    }

    fn mk_pack_fn(t: u64, n: usize) -> Code {
        Code::new(vec![
            Instruction::Pack(t, n),
            Instruction::Update(0),
            Instruction::Unwind,
        ])
    }

    /* CaseJump [...,t -> c,...]:i a:s d h[a:Constr t fs] m
                             c++i a:s d h                m
    */
    fn handle_case_jump(&mut self, alts: IntMap<u64, Code>) -> Result<()> {
        self.current.instructions.pop_front();

        let a = self
            .current
            .stack
            .peak_cloned()
            .ok_or(anyhow!("constr node addr missing"))?;

        let tag = match self.must_access_node(a) {
            Node::Constr(tag, _) => Ok(*tag),
            node => Err(anyhow!("expected constr node, got: {:?}", node)),
        }?;

        let c = alts
            .get(tag)
            .ok_or(anyhow!("couldn't find code to handle tag {}", tag))?;
        let c = c.0.deref().clone();

        self.current.instructions.prepend(c);

        Ok(())
    }

    /* Split n:i             a:s d h[a:Constr t [a_0,...,a_n-1]] m
    =>         i a_0:...:a_n-1:s d h                             m
     */
    fn handle_split(&mut self, n: usize) -> Result<()> {
        self.current.instructions.pop_front();

        let a = self
            .current
            .stack
            .pop_cloned()
            .ok_or(anyhow!("constr node addr missing"))?;

        let field_addrs = match self.must_access_node(a) {
            Node::Constr(_, field_addrs) => {
                if field_addrs.len() == n {
                    Ok(field_addrs.clone())
                } else {
                    Err(anyhow!(
                        "expected constructor to have {} fields, got {}",
                        n,
                        field_addrs.len()
                    ))
                }
            }
            node => Err(anyhow!("expected constr node, got: {:?}", node)),
        }?;

        field_addrs
            .into_iter()
            .rev()
            .for_each(|addr| self.current.stack.push(addr));

        Ok(())
    }

    fn handle_abort(&mut self) -> Result<()> {
        self.current.instructions.pop_front();

        Err(anyhow!("user code aborts"))
    }

    // Helpers

    fn unbox_boolean_at(&self, a: Addr) -> Result<bool> {
        let node = self.must_access_node(a);
        try {
            let d = WHNFData::from_node(node.clone())?;
            let res = d.unbox_boolean()?;
            res
        }
        .ok_or(anyhow!("unrecognized boolean: node {:?} at {:?}", node, a))
    }

    fn alloc_num_node(&mut self, i: i64) -> Addr {
        let entry = GlobalEntry::Num(i);
        self.globals.get(&entry).cloned().unwrap_or_else(|| {
            let addr = self.heap.alloc(Node::Num(i));
            self.globals.insert(entry, addr);
            addr
        })
    }

    fn lookup_global_name(&self, name: ast::Name) -> Result<Addr> {
        let entry = GlobalEntry::Name(name);
        self.globals
            .get(&entry)
            .map(|x| *x)
            .ok_or(anyhow!("global not found: {:?}", entry))
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

    fn pop_n_verify(&mut self, n: usize) -> Result<Vec<Addr>> {
        let popped = self.current.stack.pop_n_cloned(n);
        if popped.len() != n {
            Err(anyhow!("expected at least {} operands on the stack", n))
        } else {
            Ok(popped)
        }
    }

    fn try_pop_eval_frame(&mut self) -> bool {
        if let Some(f) = self.dump.pop() {
            self.current = f;
            true
        } else {
            false
        }
    }

    // Test Helpers
    fn follow_indirection(&self, a: Addr) -> Addr {
        let mut a = a;

        loop {
            match self.must_access_node(a) {
                Node::Indirect(next) => a = *next,
                _ => break,
            };
        }

        a
    }

    pub(super) fn inspect_global(&self, name: ast::Name) -> Result<Node> {
        let addr = self.lookup_global_name(name)?;
        let addr = self.follow_indirection(addr);
        let node = self.must_access_node(addr);
        Ok(node.clone())
    }

    // Pretty Printing
    pub fn pp<'b, D, A>(&'b self, a: &'b D) -> DocBuilder<'b, D, A>
    where
        D: DocAllocator<'b, A>,
        D::Doc: Clone,
        A: Clone,
    {
        a.concat([
            a.text("Code:"),
            a.hardline()
                .append(self.pp_instruction_stream_in(a, &self.current, None))
                .nest(2),
            a.hardline(),
            a.text("Stack:"),
            a.hardline()
                .append(self.pp_stack_in(a, &self.current, true))
                .nest(2),
            a.hardline(),
            a.text("Dump:"),
            a.hardline().append(self.pp_dump(a)).nest(2),
            a.hardline(),
            a.text("Stats:"),
            a.hardline().append(self.stats.pp(a)).nest(2),
        ])
    }

    fn pp_dump<'b, D, A>(&'b self, a: &'b D) -> DocBuilder<'b, D, A>
    where
        D: DocAllocator<'b, A>,
        D::Doc: Clone,
        A: Clone,
    {
        a.concat([
            a.text("["),
            self.dump
                .iter()
                .rev()
                .fold(a.nil(), |acc, x| {
                    acc.append(a.hardline().append(self.show_dump_item(a, x)))
                })
                .group()
                .nest(2),
            a.hardline(),
            a.text("]"),
        ])
    }

    fn show_dump_item<'b, D, A>(&'b self, a: &'b D, frame: &'b EvalFrame) -> DocBuilder<'b, D, A>
    where
        D: DocAllocator<'b, A>,
        D::Doc: Clone,
        A: Clone,
    {
        a.concat([
            a.text("<"),
            a.concat([
                a.hardline(),
                a.text("Code:").append(
                    a.hardline()
                        .append(self.pp_instruction_stream_in(a, &frame, Some(3)))
                        .nest(2),
                ),
                a.hardline(),
                a.text("Stack:").append(
                    a.hardline()
                        .append(self.pp_stack_in(a, &frame, false))
                        .nest(2),
                ),
            ])
            .group()
            .nest(2),
            a.hardline(),
            a.text(">"),
        ])
    }

    fn pp_instruction_stream_in<'b, D, A>(
        &'b self,
        a: &'b D,
        frame: &'b EvalFrame,
        limit: Option<usize>,
    ) -> DocBuilder<'b, D, A>
    where
        D: DocAllocator<'b, A>,
        D::Doc: Clone,
        A: Clone,
    {
        Instruction::pp_multi(a, frame.instructions.iter(), limit)
    }

    fn pp_stack_in<'a, 'b, D, A>(
        &'a self,
        a: &'b D,
        frame: &'b EvalFrame,
        show_items: bool,
    ) -> DocBuilder<'b, D, A>
    where
        'b: 'a,
        D: DocAllocator<'b, A>,
        D::Doc: Clone,
        A: Clone,
    {
        frame
            .stack
            .pp_with(a, |addr| self.pp_stack_item(a, *addr, show_items))
    }

    fn pp_stack_item<'b, D, A>(&self, a: &'b D, addr: Addr, show_item: bool) -> DocBuilder<'b, D, A>
    where
        D: DocAllocator<'b, A>,
        D::Doc: Clone,
        A: Clone,
    {
        a.concat(iter::once(addr.pp(a)).chain(show_item.then_some(a.concat([
            a.text(":"),
            a.space(),
            self.pp_node_at(a, addr),
        ]))))
    }

    fn pp_node_at<'b, D, A>(&self, a: &'b D, addr: Addr) -> DocBuilder<'b, D, A>
    where
        D: DocAllocator<'b, A>,
        D::Doc: Clone,
        A: Clone,
    {
        match self.must_access_node(addr) {
            Node::Num(i) => a.as_string(i),
            Node::Ap(a_1, a_2) => {
                a.concat([a.text("Ap"), a.space(), a_1.pp(a), a.space(), a_2.pp(a)])
            }
            Node::Global(_, _) => {
                let name = self
                    .globals
                    .iter()
                    .find(|(_, a)| **a == addr)
                    .map(|(e, _)| match e {
                        GlobalEntry::Name(name) => (*name.0).clone(),
                        GlobalEntry::Pack(tag, arity) => format!("Pack {} {}", tag, arity),
                        _ => panic!("unexpected number in globals at addr {:?}", addr),
                    })
                    .unwrap_or("???".to_string());

                a.concat([a.text("Global"), a.space(), a.text(name)])
            }
            Node::Indirect(_) => a
                .concat([
                    a.text("Ind"),
                    a.line(),
                    self.pp_indirect_trail(a, addr).nest(2),
                ])
                .group(),
            Node::Constr(tag, addrs) => a.concat([
                a.text("Cons"),
                a.space(),
                a.as_string(tag),
                a.space(),
                a.text("["),
                a.intersperse(addrs.iter().map(|x| x.pp(a)), a.text(", ")),
                a.text("]"),
            ]),
        }
    }

    fn pp_indirect_trail<'b, D, A>(&self, a: &'b D, addr: Addr) -> DocBuilder<'b, D, A>
    where
        D: DocAllocator<'b, A>,
        D::Doc: Clone,
        A: Clone,
    {
        let mut addrs = vec![];
        let mut next_addr = Some(addr);
        let mut last_addr = addr;

        while let Some(addr) = next_addr {
            addrs.push(addr);
            last_addr = addr;
            next_addr = match self.must_access_node(addr) {
                Node::Indirect(addr) => Some(*addr),
                _ => None,
            };
        }

        a.intersperse(
            addrs.into_iter().map(|addr| addr.pp(a)),
            a.space().append(a.text("->")).append(a.space()),
        )
        .append(
            a.line()
                .flat_alt(a.concat([a.space(), a.text("|"), a.space()]))
                .append(self.pp_node_at(a, last_addr))
                .nest(2),
        )
        .group()
    }
}

#[derive(Debug)]
pub struct MachineIter(MachineIterInternal);

impl MachineIter {
    pub fn new(m: Machine) -> Self {
        Self(MachineIterInternal::new(m))
    }
}

impl Iterator for MachineIter {
    type Item = Result<Machine>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

// Machine => Machine
// Machine => Done
// Machine => Error
// Error => Done
#[derive(Debug)]
enum MachineIterInternal {
    Machine(Machine),
    Error(anyhow::Error),
    Done,
}

impl MachineIterInternal {
    fn new(m: Machine) -> Self {
        Self::Machine(m)
    }
}

impl Iterator for MachineIterInternal {
    type Item = Result<Machine>;

    fn next(&mut self) -> Option<Self::Item> {
        let current_self = mem::replace(self, Self::Done);

        match current_self {
            Self::Machine(current_machine) => {
                if !current_machine.done() {
                    let mut next_machine = current_machine.clone();
                    let res = next_machine.step();

                    let next_iter = match res {
                        Ok(()) => Self::Machine(next_machine),
                        Err(err) => Self::Error(err),
                    };

                    *self = next_iter;
                }

                Some(Ok(current_machine))
            }
            Self::Error(err) => Some(Err(err)),
            Self::Done => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use anyhow::{Context, Ok, Result, anyhow, bail, ensure};
    use chumsky::Parser;
    use lazy_static::lazy_static;
    use pretty::Arena;
    use stacksafe::{StackSafe, stacksafe};

    use crate::{
        g_machine::{
            compiler::p,
            machine::{Machine, MachineIter, Node},
            prelude::link_with_prelude,
        },
        lexer::token_vec,
        parser::{ast, parser},
    };

    lazy_static! {
        static ref DEBUG: bool = env::var("DEBUG").is_ok_and(|x| match x.as_str() {
            "1" => true,
            x => x.to_lowercase().as_str() == "true",
        });
    }

    #[derive(Debug)]
    enum ExpectedResult {
        Num(i64),
        Constr(u64, Vec<StackSafe<ExpectedResult>>),
    }

    #[stacksafe]
    fn check_result(m: &Machine, node: &Node, expected: StackSafe<ExpectedResult>) -> Result<()> {
        match (node, expected.into_inner()) {
            (Node::Indirect(i), expected) => {
                check_result(m, m.must_access_node(*i), StackSafe::new(expected))
            }
            (Node::Num(actual), ExpectedResult::Num(expected)) => {
                ensure!(
                    *actual == expected,
                    "num should be {} but got {}",
                    expected,
                    actual
                );
                Ok(())
            }
            (
                Node::Constr(tag_actual, fields_actual),
                ExpectedResult::Constr(tag_expected, fields_expected),
            ) => {
                ensure!(
                    *tag_actual == tag_expected,
                    "constr tag should be {} but got {}",
                    tag_expected,
                    tag_actual
                );
                ensure!(
                    fields_actual.len() == fields_expected.len(),
                    "constr fields len should be {} but got {}",
                    fields_expected.len(),
                    fields_actual.len(),
                );

                fields_actual
                    .iter()
                    .copied()
                    .map(|a| m.must_access_node(a))
                    .zip(fields_expected)
                    .enumerate()
                    .try_for_each(|(idx, (node, expected))| {
                        check_result(m, node, expected).context(format!("constr field {}", idx))
                    })?;

                Ok(())
            }
            (node, expected) => bail!("not in whnf: node: {:?}, expected: {:?}", node, expected),
        }
    }

    // Assuming that the entry point is called "main"
    fn assert_eval_result(program: &str, expected: ExpectedResult) -> Result<()> {
        let entry_point = ast::Name::new("main");
        let tokens = token_vec()
            .parse(program)
            .into_result()
            .map_err(|errs| anyhow!("{:?}", errs).context("tokenize"))?;
        let ast = parser()
            .parse(&tokens)
            .into_result()
            .map_err(|errs| anyhow!("{:?}", errs).context("parse"))?;
        let compiled = p(&ast);
        let compiled = link_with_prelude(compiled);
        let mut machine = Machine::new(compiled, entry_point.clone());
        let machine = if *DEBUG {
            MachineIter::new(machine)
                .map(|m| -> Result<_, _> {
                    let m = m?;
                    let a = Arena::<()>::new();
                    println!("==================\n{}", m.pp(&a).pretty(80));
                    Ok(m)
                })
                .last()
                .unwrap()
        } else {
            machine.run()?;
            Ok(machine)
        }
        .context("running")?;
        let node = machine
            .inspect_global(entry_point.clone())
            .context("inspect-main")?;
        check_result(&machine, &node, StackSafe::new(expected)).context("check-result")?;
        Ok(())
    }

    #[test]
    fn basic() -> Result<()> {
        assert_eval_result("main = i 42", ExpectedResult::Num(42))?;
        assert_eval_result(
            "i1 = s k k;
                      main = i1 42",
            ExpectedResult::Num(42),
        )?;
        assert_eval_result("main = twice twice twice i 42", ExpectedResult::Num(42))?;
        Ok(())
    }

    #[test]
    fn update() -> Result<()> {
        assert_eval_result("main = twice (i i i) 42", ExpectedResult::Num(42))?;
        Ok(())
    }

    mod arithmetic {
        use super::*;

        #[test]
        fn unconditional() -> Result<()> {
            assert_eval_result("main = 21*2 + (2/2 - 1)", ExpectedResult::Num(42))?;
            assert_eval_result(
                "incr x = x + 1;
                          main = twice twice incr 0",
                ExpectedResult::Num(4),
            )?;
            Ok(())
        }

        #[test]
        fn conditional() -> Result<()> {
            assert_eval_result(
                "fac x = if x == 0 then 1 else x*fac (x - 1);
                          main = fac 5",
                ExpectedResult::Num(120),
            )?;
            assert_eval_result(
                "gcd a b = if a == b 
                                      then a 
                                      else if a < b 
                                            then gcd b a
                                            else gcd b (a - b);
                          main = gcd 6 10",
                ExpectedResult::Num(2),
            )?;
            Ok(())
        }
    }

    mod structual {
        use super::*;

        #[test]
        fn packs() -> Result<()> {
            assert_eval_result("main = Pack{1, 0}", ExpectedResult::Constr(1, vec![]))?;
            assert_eval_result(
                "main = let x = Pack{1, 0} in x",
                ExpectedResult::Constr(1, vec![]),
            )?;

            // FIXME: this doesn't work for now bcs we only evaluate main to WHNF
            // fn mk_nil() -> ExpectedResult {
            //     ExpectedResult::Constr(0, vec![])
            // }
            // fn mk_cons(x: i64, rest: ExpectedResult) -> ExpectedResult {
            //     ExpectedResult::Constr(
            //         1,
            //         vec![StackSafe::new(ExpectedResult::Num(x)), StackSafe::new(rest)],
            //     )
            // }

            // assert_eval_result(
            //     "main = let nil = Pack{0, 0};
            //                          cons = Pack{1, 2}
            //                         in cons 1 (force (cons 2 (force (cons 3 nil))))",
            //     mk_cons(1, mk_cons(2, mk_cons(3, mk_nil()))),
            // );
            Ok(())
        }

        #[test]
        fn pattern_matching() -> Result<()> {
            assert_eval_result(
                "sum xs = case xs of 
                                    [0] -> 0;
                                    [1] x xs -> x + sum xs;
                          nil = Pack{0,0};
                          cons = Pack{1,2};
                          l = cons 1 (cons 2 (cons 3 nil));
                          main = sum l
                         ",
                ExpectedResult::Num(6),
            )?;
            Ok(())
        }
    }
}
