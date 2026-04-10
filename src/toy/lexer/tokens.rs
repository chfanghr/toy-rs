use std::{iter::Enumerate, slice};

use nom::{Input, Needed};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keyword {
    Let,    // let
    Letrec, // letrec
    In,     // in
    Case,   // case
    Of,     // of
    Pack,   // Pack
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArithOp {
    Plus,     // +
    Minus,    // -
    Multiply, // *
    Divide,   // /
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelOp {
    LessOrEqualTo,    // <=
    LessThan,         // <
    EqualTo,          // ==
    NotEqualTo,       // /=,
    GreaterOrEqualTo, // >=,
    GreaterThan,      // >
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoolOp {
    And, // &&
    Or,  // ||
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Symbol {
    Comma,       // ,
    Backslash,   // \
    LBracket,    // [
    RBracket,    // ]
    LCurlyBrace, // {
    RCurlyBrace, // }
    LParen,      // (
    RParen,      // )
    Semicolon,   // ;
    Arrow,       // ->
    Bind,        // =, try right after RelOp
    ArithOp(ArithOp),
    RelOp(RelOp),
    BoolOp(BoolOp),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Keyword(Keyword),
    Var(String),
    Symbol(Symbol),
    Num(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tokens<'a>(&'a [Token]);

impl<'a> Tokens<'a> {
    pub fn new(s: &'a [Token]) -> Tokens<'a> {
        Tokens(s)
    }

    pub fn tokens(&self) -> &'a [Token] {
        self.0
    }
}

impl<'a> Input for Tokens<'a> {
    type Item = &'a Token;

    type Iter = slice::Iter<'a, Token>;

    type IterIndices = Enumerate<Self::Iter>;

    fn input_len(&self) -> usize {
        self.0.len()
    }

    fn take(&self, index: usize) -> Self {
        Self(&self.0[0..index])
    }

    fn take_from(&self, index: usize) -> Self {
        Self(&self.0[index..])
    }

    fn take_split(&self, index: usize) -> (Self, Self) {
        let (prefix, suffix) = self.0.split_at(index);
        (Self(prefix), Self(suffix))
    }

    fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool,
    {
        self.0.iter().position(predicate)
    }

    fn iter_elements(&self) -> Self::Iter {
        self.0.iter()
    }

    fn iter_indices(&self) -> Self::IterIndices {
        self.0.iter().enumerate()
    }

    fn slice_index(&self, count: usize) -> Result<usize, Needed> {
        if self.0.len() >= count {
            Ok(count)
        } else {
            Err(Needed::Unknown)
        }
    }
}
