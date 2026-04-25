use super::{machine::*, prelude::*, Addr};
use anyhow::{anyhow, bail, Context, Result};
use itertools::{Either, Itertools};

impl Machine {
    pub(super) fn dispatch_prim_op(
        &mut self,
        prim_op: PrimOp,
        arg_addrs: Vec<PrimOpArgAddr>,
    ) -> Result<PrimOpResult> {
        match prim_op {
            PrimOp::Neg => self.impl_prim_all_num_args_ret_num(arg_addrs, |[x]| Ok(-x)),
            PrimOp::Add => self.impl_prim_all_num_args_ret_num(arg_addrs, |[x, y]| Ok(x + y)),
            PrimOp::Sub => self.impl_prim_all_num_args_ret_num(arg_addrs, |[x, y]| Ok(x - y)),
            PrimOp::Mul => self.impl_prim_all_num_args_ret_num(arg_addrs, |[x, y]| Ok(x * y)),
            PrimOp::Div => self.impl_prim_all_num_args_ret_num(arg_addrs, |[x, y]| {
                if y == 0 {
                    Err(anyhow!("divide by zero"))
                } else {
                    Ok(x / y)
                }
            }),
            PrimOp::Eq => self.impl_prim_all_num_args_ret_bool(arg_addrs, |[x, y]| Ok(x == y)),
            PrimOp::Lt => self.impl_prim_all_num_args_ret_bool(arg_addrs, |[x, y]| Ok(x < y)),
            PrimOp::Gt => self.impl_prim_all_num_args_ret_bool(arg_addrs, |[x, y]| Ok(x > y)),
            PrimOp::IfThenElse => self.impl_prim_if_then_else(arg_addrs),
            PrimOp::MatchPair => self.impl_prim_match_pair(arg_addrs),
            PrimOp::MatchList => self.impl_prim_match_list(arg_addrs),
            PrimOp::Abort => Err(anyhow!("user code called abort")),
            PrimOp::Stop => Ok(PrimOpResult::Stop),
            PrimOp::Print => self.impl_prim_print(arg_addrs),
            PrimOp::Seq => self.impl_prim_seq(arg_addrs),
            PrimOp::Constr(constr_prim_op) => self.impl_prim_constr(arg_addrs, constr_prim_op),
        }
    }

    fn impl_prim_all_num_args<const N: usize, F>(
        &mut self,
        arg_addrs: Vec<PrimOpArgAddr>,
        f: F,
    ) -> Result<PrimOpResult>
    where
        F: Fn([i64; N]) -> Result<Node>,
    {
        let (to_be_evaluated, evaluated): (Vec<Addr>, Vec<Addr>) =
            arg_addrs.into_iter().partition_map(|addr| {
                let constr = if addr.is_whnf() {
                    Either::Right
                } else {
                    Either::Left
                };
                constr(addr.addr())
            });

        if let Some(addr) = to_be_evaluated.first() {
            return Ok(PrimOpResult::NeedFurtherEvaluate(*addr));
        }

        let num_args: Vec<i64> = evaluated
            .into_iter()
            .map(|addr| match self.must_get_node(addr) {
                Node::Num(n) => Ok(n.val()),
                n => Err(anyhow!("expected integer node at {:?}, got {:?}", addr, n)),
            })
            .try_collect()?;
        let num_args_arr: [i64; N] = num_args
            .try_into()
            .map_err(|v: Vec<i64>| anyhow!("expected {} args, got {}", N, v.len()))?;

        let node = f(num_args_arr)?;
        Ok(PrimOpResult::Done(node))
    }

    fn impl_prim_all_num_args_ret_num<const N: usize, F>(
        &mut self,
        arg_addrs: Vec<PrimOpArgAddr>,
        f: F,
    ) -> Result<PrimOpResult>
    where
        F: Fn([i64; N]) -> Result<i64>,
    {
        self.impl_prim_all_num_args(arg_addrs, |args| {
            f(args).map(|x| Node::Num(IntegerNode::new(x)))
        })
    }

    fn impl_prim_all_num_args_ret_bool<const N: usize, F>(
        &mut self,
        arg_addrs: Vec<PrimOpArgAddr>,
        f: F,
    ) -> Result<PrimOpResult>
    where
        F: Fn([i64; N]) -> Result<bool>,
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
        constr_prim_op: ConstrPrimOp,
    ) -> Result<PrimOpResult> {
        let arity = constr_prim_op.arity();

        let num_fields_got = arg_addrs.len();
        if num_fields_got != arity {
            bail!(
                "constructor expected {} fields, got {}",
                arity,
                num_fields_got
            );
        }

        let field_args = arg_addrs.into_iter().map(|x| x.addr()).collect::<Vec<_>>();
        let node = Node::Data(DataNode::new(constr_prim_op.tag(), field_args));
        Ok(PrimOpResult::Done(node))
    }

    fn expect_num_node_at(&self, addr: Addr) -> Result<i64> {
        match self.must_get_node(addr) {
            Node::Num(x) => Ok(x.val()),
            Node::Data(_) => Err(anyhow!("expected num node at {:?}, got data node", addr)),
            _ => panic!("BUG: not WHNF at {:?}", addr),
        }
    }

    fn expect_data_node_at(&self, addr: Addr) -> Result<(u64, &[Addr])> {
        match self.must_get_node(addr) {
            Node::Num(_) => Err(anyhow!("expected data node at {:?}, got num node", addr)),
            Node::Data(x) => Ok((x.tag(), x.field_addrs())),
            _ => panic!("BUG: not WHNF at {:?}", addr),
        }
    }

    fn impl_prim_if_then_else(&mut self, arg_addrs: Vec<PrimOpArgAddr>) -> Result<PrimOpResult> {
        let [pred_addr, then_branch_addr, else_branch_addr] = arg_addrs.try_into().unwrap();

        if !pred_addr.is_whnf() {
            return Ok(PrimOpResult::NeedFurtherEvaluate(pred_addr.addr()));
        }

        let (pred_data_tag, pred_data_fields) = self
            .expect_data_node_at(pred_addr.addr())
            .context("while evaluating predicate")?;

        let next_addr = match (pred_data_tag, pred_data_fields.len()) {
            (TRUE_TAG, 0) => Ok(then_branch_addr.addr()),
            (FALSE_TAG, 0) => Ok(else_branch_addr.addr()),
            (tag, fields_len) => Err(anyhow!(
                "predicate expression didn't evaluate to boolean, tag: {}, fields len: {}",
                tag,
                fields_len
            )),
        }?;

        Ok(PrimOpResult::Done(Node::Indirect(next_addr)))
    }

    fn impl_prim_match_pair(&mut self, arg_addrs: Vec<PrimOpArgAddr>) -> Result<PrimOpResult> {
        let [pair_addr, f_addr] = arg_addrs.try_into().unwrap();

        if !pair_addr.is_whnf() {
            return Ok(PrimOpResult::NeedFurtherEvaluate(pair_addr.addr()));
        }

        let (pair_data_tag, pair_data_fields) = self.expect_data_node_at(pair_addr.addr())?;

        let (a_addr, b_addr) = match (pair_data_tag, pair_data_fields.len()) {
            (PAIR_TAG, 2) => {
                let [a_addr, b_addr] = pair_data_fields.try_into().unwrap();
                Ok((a_addr, b_addr))
            }
            (tag, fields_len) => Err(anyhow!(
                "unrecognized pair constructor, tag: {}, fields len: {}",
                tag,
                fields_len
            )),
        }?;

        let ap_node_inner = Node::Ap(ApplicationNode::new(f_addr.addr(), a_addr));
        let ap_node_inner_addr = self.alloc_node(ap_node_inner);
        let node = Node::Ap(ApplicationNode::new(ap_node_inner_addr, b_addr));

        Ok(PrimOpResult::Done(node))
    }

    fn impl_prim_match_list(&mut self, arg_addrs: Vec<PrimOpArgAddr>) -> Result<PrimOpResult> {
        let [list_addr, on_nil_addr, on_cons_addr] = arg_addrs.try_into().unwrap();

        if !list_addr.is_whnf() {
            return Ok(PrimOpResult::NeedFurtherEvaluate(list_addr.addr()));
        }

        let (list_tag, list_fields) = self.expect_data_node_at(list_addr.addr())?;

        let next = match (list_tag, list_fields.len()) {
            (NIL_TAG, 0) => Ok(Either::Left(())),
            (CONS_TAG, 2) => {
                let [head_addr, tail_addr] = list_fields.try_into().unwrap();
                Ok(Either::Right((head_addr, tail_addr)))
            }
            (tag, fields_len) => Err(anyhow!(
                "unrecognized list constructor, tag: {}, fields len: {}",
                tag,
                fields_len
            )),
        }?;

        Ok(PrimOpResult::Done(match next {
            Either::Left(_) => Node::Indirect(on_nil_addr.addr()),
            Either::Right((head_addr, tail_addr)) => {
                let ap_node_inner = Node::Ap(ApplicationNode::new(on_cons_addr.addr(), head_addr));
                let ap_node_inner_addr = self.alloc_node(ap_node_inner);
                let node = Node::Ap(ApplicationNode::new(ap_node_inner_addr, tail_addr));
                node
            }
        }))
    }

    fn impl_prim_print(&mut self, arg_addrs: Vec<PrimOpArgAddr>) -> Result<PrimOpResult> {
        let [a, b] = arg_addrs.try_into().unwrap();

        if !a.is_whnf() {
            return Ok(PrimOpResult::NeedFurtherEvaluate(a.addr()));
        }

        let n = self
            .expect_num_node_at(a.addr())
            .context("cannot print data node")?;

        self.push_output(n);

        Ok(PrimOpResult::Done(Node::Indirect(b.addr())))
    }

    fn impl_prim_seq(&mut self, arg_addrs: Vec<PrimOpArgAddr>) -> Result<PrimOpResult> {
        let [a, b] = arg_addrs.try_into().unwrap();

        Ok(if a.is_whnf() {
            PrimOpResult::Done(Node::Indirect(b.addr()))
        } else {
            PrimOpResult::NeedFurtherEvaluate(a.addr())
        })
    }
}
