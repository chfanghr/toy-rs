pub mod tokens;

use tokens::*;

use itertools::Either;
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

mod new_gen {
    use std::iter;

    use chumsky::{
        error::Cheap,
        extra,
        prelude::{any, choice, end, just},
        text, IterParser, Parser,
    };
    use itertools::Either;

    use crate::lexer::tokens::{ArithOp, BoolOp, Keyword, RelOp, Symbol, Token};

    fn num<'src>() -> impl Parser<'src, &'src str, u32, extra::Err<Cheap>> {
        text::int(10).try_map(|s: &str, span| s.parse::<u32>().map_err(|_| Cheap::new(span)))
    }

    fn var_or_keyword<'src>(
    ) -> impl Parser<'src, &'src str, Either<Keyword, String>, extra::Err<Cheap>> {
        use Keyword::*;
        let head = just('_').or(any().filter(char::is_ascii_alphabetic));
        let tail = just('_').or(any().filter(char::is_ascii_alphanumeric));
        head.then(tail.repeated().collect::<Vec<_>>())
            .map(|(head, tail)| {
                let full = iter::once(head).chain(tail).collect::<String>();
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
    }

    fn symbol<'src>() -> impl Parser<'src, &'src str, Symbol, extra::Err<Cheap>> {
        use Symbol::*;
        choice((
            just(',').to(Comma),
            just('\\').to(Backslash),
            just('[').to(LBracket),
            just(']').to(RBracket),
            just('{').to(LCurlyBrace),
            just('}').to(RCurlyBrace),
            just('(').to(LParen),
            just(')').to(RParen),
            just(';').to(Semicolon),
            just("->").to(Arrow),
            rel_op().map(RelOp),
            just("=").to(Bind),
            arith_op().map(ArithOp),
            bool_op().map(BoolOp),
        ))
    }

    fn bool_op<'src>() -> impl Parser<'src, &'src str, BoolOp, extra::Err<Cheap>> {
        use BoolOp::*;
        choice((just("||").to(Or), just("&&").to(And)))
    }

    fn arith_op<'src>() -> impl Parser<'src, &'src str, ArithOp, extra::Err<Cheap>> {
        use ArithOp::*;
        choice((
            just('+').to(Plus),
            just('-').to(Minus),
            just('*').to(Multiply),
            just('/').to(Divide),
        ))
    }

    fn rel_op<'src>() -> impl Parser<'src, &'src str, RelOp, extra::Err<Cheap>> {
        use RelOp::*;
        choice((
            just("<=").to(LessOrEqualTo),
            just("<").to(LessThan),
            just("==").to(EqualTo),
            just("/=").to(NotEqualTo),
            just(">=").to(GreaterOrEqualTo),
            just(">").to(GreaterThan),
        ))
    }

    fn token<'src>() -> impl Parser<'src, &'src str, Token, extra::Err<Cheap>> {
        choice((
            num().map(Token::Num),
            var_or_keyword().map(|r| r.either(Token::Keyword, Token::Var)),
            symbol().map(Token::Symbol),
        ))
    }

    pub fn token_vec<'src>() -> impl Parser<'src, &'src str, Vec<Token>, extra::Err<Cheap>> {
        token()
            .padded()
            .repeated()
            .collect::<Vec<Token>>()
            .then_ignore(end())
    }

    #[test]
    fn test() {
        println!(
            "{:?}",
            token_vec().parse(
                "fix f = letrec x = f x in x; 
                 maybe d f m = 
                    case m of
                        [0] -> d;
                        [1] x -> f x;
                 true = Pack{0,0};
                 and = foldr (&&) true;
                 lt = Pack{0,0};
                 eq = Pack{1,0};
                 gt = Pack{2,0};
                 ifThenElse cond t f = 
                    case cond of 
                        [0] -> t;
                        [1] -> f;
                 compare l r = 
                    ifThenElse 
                        (l == r) 
                        eq 
                        (ifThenElse (l > r) gt lt);
                 main = let x = 1 + 1 + 0 / 1 * 2  in fix (k (i x));
                "
            )
        )
    }
}
