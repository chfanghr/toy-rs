use std::rc::Rc;

use monoid::Monoid;
use pretty::{DocAllocator, DocBuilder};
use serde::{Deserialize, Serialize};
use stacksafe::StackSafe;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Serialize, Deserialize)]
pub struct Name(pub Rc<String>);

impl Name {
    pub fn new(name: impl ToString) -> Name {
        Name(Rc::new(name.to_string()))
    }

    pub fn pp<'b, D, A>(&'b self, a: &'b D) -> DocBuilder<'b, D, A>
    where
        D: DocAllocator<'b, A>,
    {
        a.text(self.0.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Integer(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arity(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Constructor {
    pub tag: Tag,
    pub arity: Arity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Application<T> {
    pub l: Expr<T>,
    pub r: Expr<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bind<T> {
    pub binder: T,
    pub body: Expr<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Let<T> {
    pub is_recursive: bool,
    pub definitions: Vec<Bind<T>>,
    pub body: Expr<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch<T> {
    pub tag: Tag,
    pub bound_fields: Vec<Name>,
    pub body: Expr<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Case<T> {
    pub scru: Expr<T>,
    pub branches: Vec<Branch<T>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LamdaAbstraction<T> {
    pub arguments: Vec<T>,
    pub body: Expr<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IfThenElse<T> {
    pub pred: Expr<T>,
    pub then_branch: Expr<T>,
    pub else_branch: Expr<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Expr<T> {
    Var(Name),
    Num(Integer),
    Constr(Constructor),
    Ap(Box<StackSafe<Application<T>>>),
    Let(Box<StackSafe<Let<T>>>),
    Case(Box<StackSafe<Case<T>>>),
    Lam(Box<StackSafe<LamdaAbstraction<T>>>),
    IfThenElse(Box<StackSafe<IfThenElse<T>>>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuperCombinator<T> {
    pub name: Name,
    pub arguments: Vec<T>,
    pub body: Expr<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program<T>(pub Vec<SuperCombinator<T>>);

impl<T> Monoid for Program<T> {
    fn id() -> Self {
        Self(vec![])
    }

    fn op(self, other: Self) -> Self {
        Self(self.0.into_iter().chain(other.0).collect())
    }
}

pub fn ap_chain(exprs: Vec<Expr<Name>>) -> Expr<Name> {
    match exprs.len() {
        0 => panic!("BUG: misused ap_chain: must provide more than one expr"),
        1 => {
            let [expr] = exprs.try_into().unwrap();
            expr
        }
        _ => {
            let mut exprs = exprs;
            let [x1, x2] = exprs.drain(..2).collect::<Vec<_>>().try_into().unwrap();
            let xs = exprs;
            xs.into_iter().fold(
                Expr::Ap(Box::new(StackSafe::new(Application { l: x1, r: x2 }))),
                |inner, x| Expr::Ap(Box::new(StackSafe::new(Application { l: inner, r: x }))),
            )
        }
    }
}
