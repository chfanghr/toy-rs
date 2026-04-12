pub mod tokens;

use tokens::*;

use either::Either;
use nom::{
    branch::alt,
    bytes::tag,
    character::{
        char,
        complete::{alphanumeric0, multispace0},
        satisfy,
    },
    combinator::{all_consuming, complete, value},
    multi::separated_list0,
    sequence::delimited,
    AsChar, IResult, Parser,
};
use std::iter;

fn var_or_keyword(i: &str) -> IResult<&str, Either<Keyword, String>> {
    use Keyword::*;
    (char('_').or(satisfy(AsChar::is_alpha)))
        .and(alphanumeric0::<&str, _>)
        .map(|(x, xs)| {
            let full = iter::once(x).chain(xs.chars()).collect::<String>();
            let kw = match full.as_str() {
                "let" => Some(Let),
                "letrec" => Some(Letrec),
                "in" => Some(In),
                "case" => Some(Case),
                "of" => Some(Of),
                "Pack" => Some(Pack),
                _ => None,
            };
            kw.map(Either::Left).unwrap_or(Either::Right(full))
        })
        .parse(i)
}

fn arith_op(i: &str) -> IResult<&str, ArithOp> {
    use ArithOp::*;
    alt([
        value(Plus, char('+')),
        value(Minus, char('-')),
        value(Multiply, char('*')),
        value(Divide, char('/')),
    ])
    .parse(i)
}

fn rel_op(i: &str) -> nom::IResult<&str, RelOp> {
    use RelOp::*;
    alt([
        value(LessOrEqualTo, tag("<=")),
        value(LessThan, tag("<")),
        value(EqualTo, tag("==")),
        value(NotEqualTo, tag("/=")),
        value(GreaterOrEqualTo, tag(">=")),
        value(GreaterThan, tag(">")),
    ])
    .parse(i)
}

fn bool_op(inp: &str) -> IResult<&str, BoolOp> {
    use BoolOp::*;
    alt([value(And, tag("||")), value(Or, tag("&&"))]).parse(inp)
}

fn symbol(i: &str) -> IResult<&str, Symbol> {
    use Symbol::*;
    alt([
        value(Comma, char(',')),
        value(Backslash, char('\\')),
        value(LBracket, char('[')),
        value(RBracket, char(']')),
        value(LCurlyBrace, char('{')),
        value(RCurlyBrace, char('}')),
        value(LParen, char('(')),
        value(RParen, char(')')),
        value(Semicolon, char(';')),
    ])
    .or(value(Arrow, tag("->")))
    .or(rel_op.map(Symbol::RelOp))
    .or(arith_op.map(Symbol::ArithOp))
    .or(value(Bind, char('=')))
    .or(bool_op.map(Symbol::BoolOp))
    .parse(i)
}

fn num(i: &str) -> IResult<&str, u32> {
    nom::character::complete::u32.parse(i)
}

fn token(i: &str) -> IResult<&str, Token> {
    var_or_keyword
        .map(|e| e.either(|l| Token::Keyword(l), |r| Token::Var(r)))
        .or(symbol.map(Token::Symbol))
        .or(num.map(Token::Num))
        .parse(i)
}

pub fn token_vec(i: &str) -> nom::IResult<&str, Vec<Token>> {
    all_consuming(delimited(
        multispace0,
        separated_list0(multispace0, complete(token)),
        multispace0,
    ))
    .parse(i)
}
