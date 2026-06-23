use crate::{
    g_machine::{
        compiler::{self, PRIM_LAZY_IF},
        types::{Code, CompiledProgram, Instruction},
    },
    parser::{
        PRIM_ADD_NAME, PRIM_BOOLEAN_AND_NAME, PRIM_BOOLEAN_OR_NAME, PRIM_DIV_NAME, PRIM_EQ_NAME,
        PRIM_GE_NAME, PRIM_GT_NAME, PRIM_LE_NAME, PRIM_LT_NAME, PRIM_MUL_NAME, PRIM_NE_NAME,
        PRIM_NEG, PRIM_SUB_NAME, ast, must_lex_and_parse_sc,
    },
};

fn ski() -> Vec<ast::SuperCombinator<ast::Name>> {
    vec![
        must_lex_and_parse_sc("i x = x"),
        must_lex_and_parse_sc("k x y = x"),
        must_lex_and_parse_sc("k1 x y = y"),
        must_lex_and_parse_sc("s f g x = f x (g x)"),
    ]
}

fn primitives() -> Vec<ast::SuperCombinator<ast::Name>> {
    vec![
        must_lex_and_parse_sc(format!("{} a = 0 - a", PRIM_NEG)),
        must_lex_and_parse_sc(format!("{} a b = a + b", PRIM_ADD_NAME)),
        must_lex_and_parse_sc(format!("{} a b = a - b", PRIM_SUB_NAME)),
        must_lex_and_parse_sc(format!("{} a b = a * b", PRIM_MUL_NAME)),
        must_lex_and_parse_sc(format!("{} a b = a / b", PRIM_DIV_NAME)),
        must_lex_and_parse_sc(format!("{} a b = a && b", PRIM_BOOLEAN_AND_NAME)),
        must_lex_and_parse_sc(format!("{} a b = a || b", PRIM_BOOLEAN_OR_NAME)),
        must_lex_and_parse_sc(format!("{} a b = a == b", PRIM_EQ_NAME)),
        must_lex_and_parse_sc(format!("{} a b = a /= b", PRIM_NE_NAME)),
        must_lex_and_parse_sc(format!("{} a b = a > b", PRIM_GT_NAME)),
        must_lex_and_parse_sc(format!("{} a b = a >= b", PRIM_GE_NAME)),
        must_lex_and_parse_sc(format!("{} a b = a < b", PRIM_LT_NAME)),
        must_lex_and_parse_sc(format!("{} a b = a <= b", PRIM_LE_NAME)),
    ]
}

fn list() -> Vec<ast::SuperCombinator<ast::Name>> {
    vec![
        must_lex_and_parse_sc("nil = Pack{0,0}"),
        must_lex_and_parse_sc("cons = Pack{1,2}"),
        must_lex_and_parse_sc(
            "list onNil onCons xs =
                case xs of
                    [0] -> onNil;
                    [1] x xs -> onCons x xs
            ",
        ),
        must_lex_and_parse_sc("head = list abort k"),
        must_lex_and_parse_sc("tail = list abort k1"),
        // FIXME: use lambda when we have a lambda lifter
        must_lex_and_parse_sc("_index i x xs = if i == 0 then x else index (i - 1) xs"),
        must_lex_and_parse_sc("index i = list abort (_index i)"),
        // FIXME: use lambda when we have a lambda lifter
        must_lex_and_parse_sc("_length x xs = 1 + length xs"),
        must_lex_and_parse_sc("length = list 0 _length"),
        // FIXME: use lambda when we have a lambda lifter
        must_lex_and_parse_sc("_map f x xs = cons (f x) (map f xs)"),
        must_lex_and_parse_sc("map f = list nil (_map f)"),
        // FIXME: use lambda when we have a lambda lifter
        must_lex_and_parse_sc("_sum x xs = x + sum xs"),
        must_lex_and_parse_sc("sum = list 0 _sum"),
        // FIXME: use lambda when we have a lambda lifter
        must_lex_and_parse_sc("_filter f x xs = (if f x then cons x else i) (filter f xs)"),
        must_lex_and_parse_sc("filter f = list nil (_filter f)"),
        // FIXME: use lambda when we have a lambda lifter
        must_lex_and_parse_sc("_nats x = cons x (_nats (x + 1))"),
        must_lex_and_parse_sc("nats = _nats 0"),
    ]
}

fn misc() -> Vec<ast::SuperCombinator<ast::Name>> {
    vec![
        must_lex_and_parse_sc("fix f = letrec x = f x in x"),
        must_lex_and_parse_sc("true = Pack{1, 0}"),
        must_lex_and_parse_sc("false = Pack{0, 0}"),
        must_lex_and_parse_sc("twice f x = f (f x)"),
        must_lex_and_parse_sc(format!(
            "{} pred thenBranch elseBranch = if pred then thenBranch else elseBranch",
            PRIM_LAZY_IF
        )),
    ]
}

fn all() -> ast::Program<ast::Name> {
    ast::Program(
        ski()
            .into_iter()
            .chain(primitives())
            .chain(misc())
            .chain(list())
            .collect(),
    )
}

fn all_compiled() -> CompiledProgram {
    compiler::p(&all())
}

fn manual() -> CompiledProgram {
    CompiledProgram::new([
        (
            ast::Name::new("abort"),
            (0, Code::new(vec![Instruction::Abort])),
        ),
        (
            ast::Name::new("seq"),
            (
                2,
                Code::new(vec![
                    Instruction::Push(0),
                    Instruction::Eval,
                    Instruction::Update(2),
                    Instruction::Pop(1),
                    Instruction::Slide(1),
                    Instruction::Unwind,
                ]),
            ),
        ),
    ])
    .unwrap()
}

pub fn link_with_prelude(c: CompiledProgram) -> CompiledProgram {
    CompiledProgram::union_all([c, all_compiled(), manual()]).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prelude_can_compile() {
        let _ = dbg!(all_compiled());
    }
}
