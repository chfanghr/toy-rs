use std::{
    collections::{HashMap, VecDeque},
    iter, mem,
};

use anyhow::{Context, Result, anyhow, bail};
use itertools::{
    Either::{Left, Right},
    Itertools,
};

use crate::{
    g_machine::types::{Code, CompiledProgram, Instruction},
    parser::ast,
    utils::{
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum GlobalEntry {
    Name(ast::Name),
    Num(i64),
}

#[derive(Debug, Clone)]
struct EvalFrame {
    instructions: VecDeque<Instruction>,
    stack: Stack<Addr>,
}

#[derive(Debug, Clone)]
pub struct Machine {
    current: EvalFrame,
    dump: Vec<EvalFrame>,
    heap: Heap<Node>,
    globals: HashMap<GlobalEntry, Addr>,
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
            Instruction::Branch(then_branch, else_branch) => self
                .handle_branch(then_branch.into_inner(), else_branch.into_inner())
                .context("Branch"),
            _ => todo!(),
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
            Node::Num(_) => {
                self.current.instructions.pop_front();

                if self.try_pop_eval_frame() {
                    self.current.stack.push(a);
                }
            }
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
                                Node::Ap(l, _) => Left(*l),
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

    // Helpers

    // FIXME: constr please
    fn unbox_boolean_at(&self, a: Addr) -> Result<bool> {
        match self.must_access_node(a) {
            Node::Num(0) => Ok(false),
            Node::Num(1) => Ok(true),
            node => Err(anyhow!("unrecognized boolean: node {:?} at {:?}", node, a)),
        }
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
