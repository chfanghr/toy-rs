use crate::parser::ast;
use anyhow::{anyhow, bail, Result};

use super::{assoc::Assoc, machine::*, Addr};

impl Machine {
    pub(super) fn dispatch_node(&mut self, node_addr: Addr, node: Node) -> Result<()> {
        match node {
            Node::Num(num_node) => self.handle_num_node(node_addr, num_node),
            Node::Data(data_node) => self.handle_data_node(node_addr, data_node),
            Node::Ap(ap_node) => self.handle_ap_node(node_addr, ap_node),
            Node::SuperComb(super_comb_node) => {
                self.handle_super_comb_node(node_addr, super_comb_node)
            }
            Node::Prim(prim_node) => self.handle_prim_node(node_addr, prim_node),
            Node::Indirect(addr) => self.handle_indirect_node(node_addr, addr),
        }
    }

    fn handle_indirect_node(&mut self, node_addr: Addr, addr: Addr) -> Result<()> {
        self.assert_pop_stack(node_addr);
        self.push_stack(addr);
        Ok(())
    }

    fn handle_num_node(&mut self, _node_addr: Addr, _n: IntegerNode) -> Result<()> {
        bail!("cannot apply to an integer")
    }

    fn handle_data_node(&mut self, _node_addr: Addr, _n: DataNode) -> Result<()> {
        bail!("cannot apply to a data node")
    }

    fn handle_ap_node(&mut self, node_addr: Addr, n: ApplicationNode) -> Result<()> {
        let r = self.follow_indirect(n.r_addr());
        if r != n.r_addr() {
            self.replace_node_at(node_addr, Node::Ap(ApplicationNode::new(n.l_addr(), r)));
        }
        self.push_stack(n.l_addr());
        Ok(())
    }

    fn handle_super_comb_node(&mut self, node_addr: Addr, n: SuperCombinatorNode) -> Result<()> {
        self.assert_pop_stack(node_addr);

        let num_args = n.inner().arguments.len();
        let ap_node_addrs = self.pop_stack_n(num_args);
        if num_args != ap_node_addrs.len() {
            Err(anyhow!(
                "super combinator {:?} expected {:} args, got {:}",
                n,
                num_args,
                ap_node_addrs.len()
            ))?
        }
        let node_addr_to_update = if num_args == 0 {
            node_addr // Constant Applicative Form
        } else {
            ap_node_addrs.last().copied().unwrap()
        };

        let env_args = ap_node_addrs
            .iter()
            .zip(n.inner().arguments.clone())
            .map(|(addr, name)| {
                let r_addr = self.must_get_application_node_r_at(*addr);
                (name, r_addr)
            })
            .fold(Assoc::new(), |mut a, (name, addr)| {
                a.insert(name, addr);
                a
            });
        let env = Assoc::combine(self.globals().clone(), env_args);
        let addr = self.instantiate(&env, &n.inner().body, Some(node_addr_to_update))?;

        assert_eq!(addr, node_addr_to_update);
        self.push_stack(addr);

        Ok(())
    }

    fn instantiate(
        &mut self,
        env: &Assoc<ast::Name, Addr>,
        expr: &ast::Expr<ast::Name>,
        // To handle recursive let where we need to know the binder's addr before instatiating the respective expression.
        replace_at: Option<Addr>,
    ) -> Result<Addr> {
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
                .ok_or(anyhow!("variable {:?} not found", v))
                .map(|a| self.replace_or_alloc_node_at(replace_at, Node::Indirect(a))),
            ast::Expr::Let(l) => {
                let preallocated_binders = l.is_recursive.then(|| {
                    l.definitions
                        .iter()
                        .map(|b| (b.binder.clone(), self.alloc_uninitialized_node()))
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
                    .collect::<Result<Vec<(ast::Name, Addr)>>>()?
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

    fn handle_prim_node(&mut self, node_addr: Addr, prim_node: PrimNode) -> Result<()> {
        let height_to_restore = self.stack_height();

        self.assert_pop_stack(node_addr);
        let arity = prim_node.prim_op().get_arity();
        let ap_node_addrs = self.pop_stack_n(arity);
        let num_popped = ap_node_addrs.len();

        if num_popped != arity {
            bail!(
                "prim op {:?} expected {} args, got {}",
                prim_node.prim_op(),
                arity,
                num_popped
            )
        }

        let node_addr_to_override = if arity == 0 {
            node_addr // can be CAF also
        } else {
            *ap_node_addrs.last().unwrap()
        };

        let arg_addrs = ap_node_addrs
            .iter()
            .copied()
            .map(|addr| {
                let arg_addr = self.must_get_application_node_r_at(addr);
                let node = self.must_get_node(arg_addr);
                if node.is_data_node() {
                    PrimOpArgAddr::DataOrNum(arg_addr)
                } else {
                    PrimOpArgAddr::Other(arg_addr)
                }
            })
            .collect::<Vec<_>>();

        let res = self.dispatch_prim_op(prim_node.prim_op().clone(), arg_addrs)?;

        match res {
            PrimOpResult::NeedFurtherEvaluate(eval_addr) => {
                self.set_stack_height(height_to_restore);
                self.push_stack_frame();
                self.push_stack(eval_addr);
            }
            PrimOpResult::Done(node) => {
                self.replace_node_at(node_addr_to_override, node);
                self.push_stack(node_addr_to_override);
            }
            PrimOpResult::Stop => self.clear_stack(),
        };

        Ok(())
    }
}
