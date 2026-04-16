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
    Subtract, // -
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
