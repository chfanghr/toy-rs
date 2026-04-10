pub mod ast;

use crate::lexer::tokens::*;
use ast::*;
use nom::{
    bytes::take,
    combinator::{complete, value},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, separated_pair, terminated},
    IResult, Parser,
};

// MARK: Program

pub fn program(i: Tokens) -> IResult<Tokens, Program<Name>> {
    separated_list1(is_symbol(Symbol::Semicolon), complete(super_comb))
        .map(Program)
        .parse(i)
}

// MARK: Supercombinators

// sc -> var var1 .... varN = expr
fn super_comb(i: Tokens) -> IResult<Tokens, SuperCombinator<Name>> {
    separated_pair((var_token, many0(var_token)), is_symbol(Symbol::Bind), expr)
        .map(|((name, arguments), body)| SuperCombinator {
            name,
            arguments,
            body,
        })
        .parse(i)
}

// MARK: Expr

// expr -> let defns in expr
//       | letrec defns in expr
//       | case expr of alts
//       | \ var1. . . varn -> expr
//       | expr1
fn expr(i: Tokens) -> nom::IResult<Tokens, Expr<Name>> {
    (let_in.map(boxed(Expr::Let)))
        .or(case_of.map(boxed(Expr::Case)))
        .or(lambda.map(boxed(Expr::Lam)))
        .or(expr1)
        .parse(i)
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

// expr1 -> expr2 || expr1
//        | expr2
fn expr1(i: Tokens) -> nom::IResult<Tokens, Expr<Name>> {
    complete(
        separated_pair(expr2, is_symbol(Symbol::BoolOp(BoolOp::Or)), expr1)
            .map(|(l, r)| ap_chain(vec![Expr::Var(Name::new("||")), l, r])),
    )
    .or(expr2)
    .parse(i)
}

// expr2 -> expr3 && expr2
//        | expr3
fn expr2(i: Tokens) -> nom::IResult<Tokens, Expr<Name>> {
    complete(
        separated_pair(expr3, is_symbol(Symbol::BoolOp(BoolOp::And)), expr2)
            .map(|(l, r)| ap_chain(vec![Expr::Var(Name::new("&&")), l, r])),
    )
    .or(expr3)
    .parse(i)
}

// expr3 -> expr4 relop expr4
//  | expr4
fn expr3(i: Tokens) -> nom::IResult<Tokens, Expr<Name>> {
    (expr4, rel_op_var, expr4)
        .map(|(l, op, r)| ap_chain(vec![op, l, r]))
        .or(expr4)
        .parse(i)
}

fn rel_op_var(i: Tokens) -> nom::IResult<Tokens, Expr<Name>> {
    match_token(|t| {
        let op = match t {
            Token::Symbol(Symbol::RelOp(op)) => Ok(op),
            _ => Err(format!("expected rel op token, got {:?}", t)),
        }?;

        Ok(Expr::Var(Name::new(match op {
            RelOp::LessOrEqualTo => "<=",
            RelOp::LessThan => "<",
            RelOp::EqualTo => "==",
            RelOp::NotEqualTo => "/=",
            RelOp::GreaterOrEqualTo => ">=",
            RelOp::GreaterThan => ">",
        })))
    })
    .parse(i)
}

// expr4 -> expr5 + expr4
//        | expr5 - expr5
//        | expr5
fn expr4(i: Tokens) -> nom::IResult<Tokens, Expr<Name>> {
    complete(
        separated_pair(expr5, is_symbol(Symbol::ArithOp(ArithOp::Plus)), expr4)
            .map(|(l, r)| ap_chain(vec![Expr::Var(Name::new("+")), l, r])),
    )
    .or(complete(
        separated_pair(expr5, is_symbol(Symbol::ArithOp(ArithOp::Minus)), expr5)
            .map(|(l, r)| ap_chain(vec![Expr::Var(Name::new("-")), l, r])),
    ))
    .or(expr5)
    .parse(i)
}

// expr5 -> expr6 * expr5
//        | expr6 / expr6
//        | expr6
fn expr5(i: Tokens) -> nom::IResult<Tokens, Expr<Name>> {
    complete(
        separated_pair(expr6, is_symbol(Symbol::ArithOp(ArithOp::Multiply)), expr5)
            .map(|(l, r)| ap_chain(vec![Expr::Var(Name::new("*")), l, r])),
    )
    .or(complete(
        separated_pair(expr6, is_symbol(Symbol::ArithOp(ArithOp::Divide)), expr6)
            .map(|(l, r)| ap_chain(vec![Expr::Var(Name::new("/")), l, r])),
    ))
    .or(expr6)
    .parse(i)
}

// expr6 -> aexpr1. . . aexprn (n >= 1)
fn expr6(i: Tokens) -> nom::IResult<Tokens, Expr<Name>> {
    many1(complete(atomic_expr)).map(ap_chain).parse(i)
}

// let defns in expr
// letrec defns in expr
fn let_in(i: Tokens) -> nom::IResult<Tokens, Let<Name>> {
    (value(false, is_keyword(Keyword::Let)).or(value(true, is_keyword(Keyword::Letrec))))
        .and(terminated(
            separated_list0(is_symbol(Symbol::Semicolon), bind),
            is_keyword(Keyword::In),
        ))
        .and(expr)
        .map(|((is_recursive, definitions), body)| Let {
            is_recursive,
            definitions,
            body,
        })
        .parse(i)
}

// defn: var = expr
fn bind(i: Tokens) -> nom::IResult<Tokens, Bind<Name>> {
    separated_pair(var_token, is_symbol(Symbol::Bind), expr)
        .map(|(binder, body)| Bind { binder, body })
        .parse(i)
}

// case expr of branches
fn case_of(i: Tokens) -> nom::IResult<Tokens, Case<Name>> {
    delimited(is_keyword(Keyword::Case), expr, is_keyword(Keyword::Of))
        .and(separated_list1(is_symbol(Symbol::Semicolon), branch))
        .map(|(scru, branches)| Case { scru, branches })
        .parse(i)
}

// [num] var1 ... varN -> expr
fn branch(i: Tokens) -> nom::IResult<Tokens, Branch<Name>> {
    terminated(
        delimited(
            is_symbol(Symbol::LBracket),
            num_token::<u64>.map(Tag),
            is_symbol(Symbol::RBracket),
        )
        .and(many0(var_token)),
        is_symbol(Symbol::Arrow),
    )
    .and(expr)
    .map(|((tag, bound_fields), body)| Branch {
        tag,
        bound_fields,
        body,
    })
    .parse(i)
}

// \var1 ... varN -> expr
fn lambda(i: Tokens) -> nom::IResult<Tokens, LamdaAbstraction<Name>> {
    (delimited(
        is_symbol(Symbol::Backslash),
        many0(var_token),
        is_symbol(Symbol::Arrow),
    )
    .and(expr))
    .map(|(arguments, body)| LamdaAbstraction { arguments, body })
    .parse(i)
}

// MARK: Atomic Expr

/*
aexpr -> var
       | num
       | Pack{num, num}
       | (expr)
*/

fn atomic_expr(i: Tokens) -> nom::IResult<Tokens, Expr<Name>> {
    (var.map(Expr::Var))
        .or(num.map(Expr::Num))
        .or(constr.map(Expr::Constr))
        .or(delimited(
            is_symbol(Symbol::LParen),
            expr,
            is_symbol(Symbol::RParen),
        ))
        .parse(i)
}

// Variable
fn var(i: Tokens) -> nom::IResult<Tokens, Name> {
    var_token.parse(i)
}

// Number
fn num(i: Tokens) -> nom::IResult<Tokens, Integer> {
    num_token.map(Integer).parse(i)
}

// Constructor: Pack{num, num}
fn constr(i: Tokens) -> nom::IResult<Tokens, Constructor> {
    delimited(
        is_keyword(Keyword::Pack).and(is_symbol(Symbol::LCurlyBrace)),
        separated_pair(num_token::<u64>, is_symbol(Symbol::Comma), num_token::<u64>),
        is_token(Token::Symbol(Symbol::RCurlyBrace)),
    )
    .map(|(tag, arity)| Constructor {
        tag: Tag(tag),
        arity: Arity(arity),
    })
    .parse(i)
}

// MARK: Utils

fn is_symbol<'a>(s: Symbol) -> impl Fn(Tokens<'a>) -> nom::IResult<Tokens, ()> {
    is_token(Token::Symbol(s))
}

fn is_keyword<'a>(kw: Keyword) -> impl Fn(Tokens<'a>) -> nom::IResult<Tokens, ()> {
    is_token(Token::Keyword(kw))
}

fn is_token<'a>(tt: Token) -> impl Fn(Tokens<'a>) -> nom::IResult<Tokens, ()> {
    match_token(move |t| {
        if t == &tt {
            Ok(())
        } else {
            Err(format!("expected token {:?}, got {:?}", tt, t))
        }
    })
}

fn num_token<'a, O: From<u32>>(i: Tokens<'a>) -> nom::IResult<Tokens<'a>, O> {
    match_token(|t| match t {
        Token::Num(x) => Ok(O::from(*x)),
        _ => Err(format!("expected a number, got {:?}", t)),
    })
    .parse(i)
}

fn var_token<'a>(i: Tokens<'a>) -> nom::IResult<Tokens<'a>, Name> {
    match_token(|t| match t {
        Token::Var(s) => Ok(Name::new(s)),
        _ => Err(format!("expected a var, got {:?}", t)),
    })
    .parse(i)
}

fn one_token<'a>(i: Tokens<'a>) -> nom::IResult<Tokens<'a>, &'a Token> {
    take(1usize)
        .map(|s: Tokens| s.tokens().get(0).unwrap())
        .parse(i)
}

fn match_token<'a, R>(
    f: impl Fn(&'a Token) -> Result<R, String>,
) -> impl Fn(Tokens<'a>) -> nom::IResult<Tokens, R> {
    move |i| complete(one_token.map_res(&f)).parse(i)
}

fn boxed<T, R>(f: impl Fn(Box<T>) -> R) -> impl Fn(T) -> R {
    move |x| f(Box::new(x))
}

// MARK: Test

#[cfg(test)]
mod test {
    use super::*;
    use crate::lexer;

    #[test]
    fn test() {
        let _ = (|| -> Result<_, String> {
            let inp = "main = i (i 42)";
            let (_rest, tokens) = lexer::token_vec.parse(inp).map_err(|err| err.to_string())?;
            println!("{:#?}", tokens);
            let tokens = Tokens::new(&tokens);
            let res = program.parse(tokens).map_err(|err| err.to_string())?;
            println!("{:#?}", res);
            Ok(())
        })()
        .unwrap();
    }
}
