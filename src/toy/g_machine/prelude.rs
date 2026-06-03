use std::collections::BTreeMap;

use crate::{
    g_machine::{compiler, types::CompiledProgram},
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

fn misc() -> Vec<ast::SuperCombinator<ast::Name>> {
    vec![must_lex_and_parse_sc("fix f = letrec x = f x in x")]
}

fn all() -> ast::Program<ast::Name> {
    ast::Program(
        ski()
            .into_iter()
            .chain(primitives())
            .chain(misc())
            .collect(),
    )
}

fn all_compiled() -> CompiledProgram {
    compiler::p(&all())
}

pub(super) fn link_with_prelude(c: CompiledProgram) -> CompiledProgram {
    let c_len = c.0.len();

    let p = all_compiled();
    let p_len = p.0.len();

    let o = p.0.into_iter().chain(c.0).collect::<BTreeMap<_, _>>();

    assert_eq!(o.len(), c_len + p_len);

    CompiledProgram::new(o)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn prelude_can_compile() {
        let _ = dbg!(all_compiled());
    }

    #[test]
    fn btree_map_dup_keys() {
        assert_eq!(
            *([("a", 0), ("a", 1)]
                .into_iter()
                .collect::<BTreeMap<&str, usize>>()
                .get("a")
                .unwrap()),
            1
        );
    }
}
