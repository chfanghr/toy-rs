#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Name(pub String);

impl Name {
    pub fn new(name: impl ToString) -> Name {
        Name(name.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Integer(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tag(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arity(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Constructor {
    pub tag: Tag,
    pub arity: Arity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Application<T> {
    pub l: Expr<T>,
    pub r: Expr<T>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bind<T> {
    pub binder: T,
    pub body: Expr<T>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Let<T> {
    pub is_recursive: bool,
    pub definitions: Vec<Bind<T>>,
    pub body: Expr<T>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch<T> {
    pub tag: Tag,
    pub bound_fields: Vec<Name>,
    pub body: Expr<T>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case<T> {
    pub scru: Expr<T>,
    pub branches: Vec<Branch<T>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LamdaAbstraction<T> {
    pub arguments: Vec<T>,
    pub body: Expr<T>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr<T> {
    Var(Name),
    Num(Integer),
    Constr(Constructor),
    Ap(Box<Application<T>>),
    Let(Box<Let<T>>),
    Case(Box<Case<T>>),
    Lam(Box<LamdaAbstraction<T>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuperCombinator<T> {
    pub name: Name,
    pub arguments: Vec<T>,
    pub body: Expr<T>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program<T>(pub Vec<SuperCombinator<T>>);

pub fn prelude() -> Vec<SuperCombinator<Name>> {
    vec![
        SuperCombinator {
            name: Name::new("i"), // id
            arguments: vec![Name::new("x")],
            body: Expr::Var(Name::new("x")),
        },
        SuperCombinator {
            name: Name::new("k"), // const
            arguments: vec![Name::new("x"), Name::new("y")],
            body: Expr::Var(Name::new("x")),
        },
        SuperCombinator {
            // <*>
            name: Name::new("s"),
            arguments: vec![
                Name::new("f"),
                Name::new("g"),
                Name::new("x"),
                Name::new("y"),
            ],
            body: Expr::Ap(Box::new(Application {
                l: Expr::Ap(Box::new(Application {
                    l: Expr::Var(Name::new("f")),
                    r: Expr::Var(Name::new("x")),
                })),
                r: Expr::Ap(Box::new(Application {
                    l: Expr::Var(Name::new("g")),
                    r: Expr::Var(Name::new("y")),
                })),
            })),
        },
    ]
}
