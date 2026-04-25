pub mod ast;

use chumsky::{
    error::Cheap,
    extra,
    pratt::{infix, left, none, right},
    prelude::{any, choice, end, just, recursive, Recursive},
    recursive::Direct,
    IterParser, Parser,
};

use crate::{
    lexer::{self, tokens::*},
    parser::ast::*,
};

// To translate operators to function names, which should be then implemented by the vm.
pub const PRIM_BOOLEAN_OR_NAME: &'static str = "_prim_boolean_or";
pub const PRIM_BOOLEAN_AND_NAME: &'static str = "_prim_boolean_and";
pub const PRIM_GT_NAME: &'static str = "_prim_gt";
pub const PRIM_GE_NAME: &'static str = "_prim_ge";
pub const PRIM_LT_NAME: &'static str = "_prim_lt";
pub const PRIM_LE_NAME: &'static str = "_prim_le";
pub const PRIM_EQ_NAME: &'static str = "_prim_eq";
pub const PRIM_NE_NAME: &'static str = "_prim_ne";
pub const PRIM_ADD_NAME: &'static str = "_prim_add";
pub const PRIM_SUB_NAME: &'static str = "_prim_sub";
pub const PRIM_MUL_NAME: &'static str = "_prim_mul";
pub const PRIM_DIV_NAME: &'static str = "_prim_div";

pub fn must_lex_and_parse_sc(inp: impl AsRef<str>) -> SuperCombinator<Name> {
    lexer::token_vec()
        .parse(inp.as_ref())
        .into_result()
        .and_then(|toks| super_comb().parse(&toks).into_result())
        .unwrap()
}

pub fn prelude() -> Vec<SuperCombinator<Name>> {
    vec![
        must_lex_and_parse_sc("i x = x"),
        must_lex_and_parse_sc("k x y = x"),
        must_lex_and_parse_sc("k1 x y = y"),
        must_lex_and_parse_sc("s f g x = f x (g x)"),
        must_lex_and_parse_sc("fix f = letrec x = f x in x"),
    ]
}

pub fn parser<'src>() -> impl Parser<'src, &'src [Token], Program<Name>, extra::Err<Cheap>> + Clone
{
    program().then_ignore(end())
}

// program -> sc_1 ; ... ; sc_n where n >= 1
fn program<'src>() -> impl Parser<'src, &'src [Token], Program<Name>, extra::Err<Cheap>> + Clone {
    super_comb()
        .separated_by(just_symbol(Symbol::Semicolon))
        .collect::<Vec<_>>()
        .map(Program)
}

// sc -> var var_1 .... var_n = expr where n >= 0
fn super_comb<'src>(
) -> impl Parser<'src, &'src [Token], SuperCombinator<Name>, extra::Err<Cheap>> + Clone {
    var()
        .then(var().repeated().collect::<Vec<_>>())
        .then_ignore(just_symbol(Symbol::Bind))
        .then(expr())
        .map(|((name, arguments), body)| SuperCombinator {
            name,
            arguments,
            body,
        })
}

fn match_token<'src, T>(
    f: impl Fn(Token) -> Option<T> + Clone,
) -> impl Parser<'src, &'src [Token], T, extra::Err<Cheap>> + Clone {
    any().try_map(move |token, span| f(token).ok_or(Cheap::new(span)))
}

fn var_token<'src>() -> impl Parser<'src, &'src [Token], String, extra::Err<Cheap>> + Clone {
    match_token(|t| match t {
        Token::Var(n) => Some(n),
        _ => None,
    })
}

fn num_token<'src, O: From<u32>>() -> impl Parser<'src, &'src [Token], O, extra::Err<Cheap>> + Clone
{
    match_token(|t| match t {
        Token::Num(n) => Some(O::from(n)),
        _ => None,
    })
}

fn just_keyword<'src>(
    k: Keyword,
) -> impl Parser<'src, &'src [Token], Token, extra::Err<Cheap>> + Clone {
    just(Token::Keyword(k))
}

fn just_symbol<'src>(
    s: Symbol,
) -> impl Parser<'src, &'src [Token], Token, extra::Err<Cheap>> + Clone {
    just(Token::Symbol(s))
}

fn just_arith_op<'src>(
    a: ArithOp,
) -> impl Parser<'src, &'src [Token], Token, extra::Err<Cheap>> + Clone {
    just(Token::Symbol(Symbol::ArithOp(a)))
}

fn just_rel_op<'src>(
    r: RelOp,
) -> impl Parser<'src, &'src [Token], Token, extra::Err<Cheap>> + Clone {
    just(Token::Symbol(Symbol::RelOp(r)))
}

fn just_bool_op<'src>(
    b: BoolOp,
) -> impl Parser<'src, &'src [Token], Token, extra::Err<Cheap>> + Clone {
    just(Token::Symbol(Symbol::BoolOp(b)))
}

// Constructor: Pack{num, num}
fn constr<'src>() -> impl Parser<'src, &'src [Token], Constructor, extra::Err<Cheap>> + Clone {
    just_keyword(Keyword::Pack)
        .ignore_then(
            ((num_token().map(Tag))
                .then_ignore(just_symbol(Symbol::Comma))
                .then(num_token().map(Arity)))
            .delimited_by(
                just_symbol(Symbol::LCurlyBrace),
                just_symbol(Symbol::RCurlyBrace),
            ),
        )
        .map(|(tag, arity)| Constructor { tag, arity })
}

// var
fn var<'src>() -> impl Parser<'src, &'src [Token], Name, extra::Err<Cheap>> + Clone {
    var_token().map(Name::new)
}

// num
fn num<'src>() -> impl Parser<'src, &'src [Token], Integer, extra::Err<Cheap>> + Clone {
    num_token().map(Integer)
}

/*
aexpr -> var
       | num
       | Pack{num, num}
       | (expr)
*/
fn aexpr<'src: 'b, 'b>(
    expr: Recursive<Direct<'src, 'b, &'src [Token], Expr<Name>, extra::Err<Cheap>>>,
) -> impl Parser<'src, &'src [Token], Expr<Name>, extra::Err<Cheap>> + 'b + Clone {
    choice((
        var().map(Expr::Var),
        num().map(Expr::Num),
        constr().map(Expr::Constr),
        expr.delimited_by(just_symbol(Symbol::LParen), just_symbol(Symbol::RParen)),
    ))
}

fn expr<'src>() -> impl Parser<'src, &'src [Token], Expr<Name>, extra::Err<Cheap>> + Clone {
    recursive(|this| {
        let aexpr_chain = aexpr(this.clone())
            .repeated()
            .at_least(1)
            .collect::<Vec<_>>()
            .map(ap_chain);
        let def_infix_op = |assoc, p, f: &'static str| {
            infix(assoc, p, |l, _, r, _| {
                ap_chain(vec![Expr::Var(Name::new(f.to_string())), l, r])
            })
        };
        let ops = aexpr_chain.clone().pratt((
            def_infix_op(
                right(2),
                just_bool_op(BoolOp::Or).boxed(),
                PRIM_BOOLEAN_OR_NAME,
            ),
            def_infix_op(
                right(3),
                just_bool_op(BoolOp::And).boxed(),
                PRIM_BOOLEAN_AND_NAME,
            ),
            def_infix_op(
                none(4),
                just_rel_op(RelOp::GreaterThan).boxed(),
                PRIM_GT_NAME,
            ),
            def_infix_op(
                none(4),
                just_rel_op(RelOp::GreaterOrEqualTo).boxed(),
                PRIM_GE_NAME,
            ),
            def_infix_op(none(4), just_rel_op(RelOp::LessThan).boxed(), PRIM_LT_NAME),
            def_infix_op(
                none(4),
                just_rel_op(RelOp::LessOrEqualTo).boxed(),
                PRIM_LE_NAME,
            ),
            def_infix_op(none(4), just_rel_op(RelOp::EqualTo).boxed(), PRIM_EQ_NAME),
            def_infix_op(
                none(4),
                just_rel_op(RelOp::NotEqualTo).boxed(),
                PRIM_NE_NAME,
            ),
            def_infix_op(left(6), just_arith_op(ArithOp::Plus).boxed(), PRIM_ADD_NAME),
            def_infix_op(
                left(6),
                just_arith_op(ArithOp::Subtract).boxed(),
                PRIM_SUB_NAME,
            ),
            def_infix_op(
                left(7),
                just_arith_op(ArithOp::Multiply).boxed(),
                PRIM_MUL_NAME,
            ),
            def_infix_op(
                left(7),
                just_arith_op(ArithOp::Divide).boxed(),
                PRIM_DIV_NAME,
            ),
        ));

        choice((
            let_in(this.clone()).map(apply_boxed(Expr::Let)).boxed(),
            case_of(this.clone()).map(apply_boxed(Expr::Case)).boxed(),
            lambda(this.clone()).map(apply_boxed(Expr::Lam)).boxed(),
            ops,
            aexpr_chain,
        ))
    })
}

// let defns in expr
// letrec defns in expr
// defns -> defn_1 ; ... ; defn_n where n >= 1
fn let_in<'src: 'b, 'b>(
    expr: Recursive<Direct<'src, 'b, &'src [Token], Expr<Name>, extra::Err<Cheap>>>,
) -> impl Parser<'src, &'src [Token], Let<Name>, extra::Err<Cheap>> + 'b + Clone {
    let is_rec = (just_keyword(Keyword::Let).to(false)).or(just_keyword(Keyword::Letrec).to(true));
    let defns = bind(expr.clone())
        .separated_by(just_symbol(Symbol::Semicolon))
        .at_least(1)
        .collect::<Vec<_>>();
    is_rec
        .then(defns)
        .then_ignore(just_keyword(Keyword::In))
        .then(expr)
        .map(|((is_recursive, definitions), body)| Let {
            is_recursive,
            definitions,
            body,
        })
}

// defn -> var = expr
fn bind<'src: 'b, 'b>(
    expr: Recursive<Direct<'src, 'b, &'src [Token], Expr<Name>, extra::Err<Cheap>>>,
) -> impl Parser<'src, &'src [Token], Bind<Name>, extra::Err<Cheap>> + 'b + Clone {
    var()
        .then_ignore(just_symbol(Symbol::Bind))
        .then(expr)
        .map(|(binder, body)| Bind { binder, body })
}

// case expr of branches
// branch -> branch_1 ; ... ; branch_n where n >= 1
fn case_of<'src: 'b, 'b>(
    expr: Recursive<Direct<'src, 'b, &'src [Token], Expr<Name>, extra::Err<Cheap>>>,
) -> impl Parser<'src, &'src [Token], Case<Name>, extra::Err<Cheap>> + 'b + Clone {
    just_keyword(Keyword::Case)
        .ignore_then(expr.clone())
        .then_ignore(just_keyword(Keyword::Of))
        .then(
            branch(expr)
                .separated_by(just_symbol(Symbol::Semicolon))
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .map(|(scru, branches)| Case { scru, branches })
}

// branch -> <num> var_1, ... , var_n -> expr where n >= 0
fn branch<'src: 'b, 'b>(
    expr: Recursive<Direct<'src, 'b, &'src [Token], Expr<Name>, extra::Err<Cheap>>>,
) -> impl Parser<'src, &'src [Token], Branch<Name>, extra::Err<Cheap>> + 'b + Clone {
    let tag = num_token()
        .delimited_by(just_symbol(Symbol::LBracket), just_symbol(Symbol::RBracket))
        .map(Tag);
    let bounded_fields = var().repeated().collect::<Vec<_>>();

    tag.then(bounded_fields)
        .then_ignore(just_symbol(Symbol::Arrow))
        .then(expr)
        .map(|((tag, bound_fields), body)| Branch {
            tag,
            bound_fields,
            body,
        })
}

// \var_1 ... var_n -> expr where n >= 1
fn lambda<'src: 'b, 'b>(
    expr: Recursive<Direct<'src, 'b, &'src [Token], Expr<Name>, extra::Err<Cheap>>>,
) -> impl Parser<'src, &'src [Token], LamdaAbstraction<Name>, extra::Err<Cheap>> + 'b + Clone {
    just_symbol(Symbol::Backslash)
        .ignore_then(var().repeated().at_least(1).collect::<Vec<_>>())
        .then_ignore(just_symbol(Symbol::Arrow))
        .then(expr)
        .map(|(arguments, body)| LamdaAbstraction { arguments, body })
}

fn ap_chain(exprs: Vec<Expr<Name>>) -> Expr<Name> {
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
                Expr::Ap(Box::new(Application { l: x1, r: x2 })),
                |inner, x| Expr::Ap(Box::new(Application { l: inner, r: x })),
            )
        }
    }
}

fn apply_boxed<T, R>(f: impl Fn(Box<T>) -> R) -> impl Fn(T) -> R {
    move |x| f(Box::new(x))
}

#[cfg(test)]
mod test {
    use crate::lexer;

    use super::*;

    #[test]
    fn test_constr() {
        let input: Vec<Token> = vec![
            Token::Keyword(Keyword::Pack),
            Token::Symbol(Symbol::LCurlyBrace),
            Token::Num(0),
            Token::Symbol(Symbol::Comma),
            Token::Num(0),
            Token::Symbol(Symbol::RCurlyBrace),
        ];
        assert_eq!(
            constr().parse(&input).into_result(),
            Ok(Constructor {
                tag: Tag(0),
                arity: Arity(0),
            }),
        );
    }

    #[test]
    fn test_parser() {
        let res = lexer::token_vec()
            .parse(
                "main = i (i (i 4)); 
                 i x = x; 
                 neg = _prim_neg
                ",
            )
            .into_result()
            .and_then(|tokens| parser().parse(&tokens).into_result());
        println!("{:?}", res);
    }
}
