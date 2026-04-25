use super::machine::PrimOpKind;
use crate::parser::{ast, must_lex_and_parse_sc, PRIM_BOOLEAN_AND_NAME, PRIM_NE_NAME};

pub(super) const FALSE_TAG: u64 = 0;
pub(super) const TRUE_TAG: u64 = 1;

pub(super) const PAIR_TAG: u64 = 0;

pub(super) const NIL_TAG: u64 = 0;
pub(super) const CONS_TAG: u64 = 1;

pub(super) const UNIT_TAG: u64 = 0;

pub(super) fn extended_prelude() -> Vec<ast::SuperCombinator<ast::Name>> {
    vec![
        must_lex_and_parse_sc(format!("neg = {}", PrimOpKind::Neg.to_name().unwrap())),
        must_lex_and_parse_sc(format!("false = Pack{{{},0}}", FALSE_TAG)),
        must_lex_and_parse_sc(format!("true = Pack{{{},0}}", TRUE_TAG)),
        must_lex_and_parse_sc(format!(
            "if = {}",
            PrimOpKind::IfThenElse.to_name().unwrap()
        )),
        must_lex_and_parse_sc(format!("{} x y = if x y false", PRIM_BOOLEAN_AND_NAME)),
        must_lex_and_parse_sc(format!("{} x y = if x true y", PRIM_BOOLEAN_AND_NAME)),
        must_lex_and_parse_sc("not x = if x false true"),
        must_lex_and_parse_sc("xor x y = if x (not y) y"),
        must_lex_and_parse_sc(format!("{} x y = not ({} x y)", PRIM_NE_NAME, PRIM_NE_NAME)),
        must_lex_and_parse_sc(format!("{} x y = (x < y) || (x == y) ", PRIM_NE_NAME)),
        must_lex_and_parse_sc(format!("{} x y = (x > y) || (x == y) ", PRIM_NE_NAME)),
        must_lex_and_parse_sc(format!("mkPair a b = Pack{{{}, 2}} a b", PAIR_TAG)),
        must_lex_and_parse_sc(format!(
            "casePair p f = {} p f",
            PrimOpKind::MatchPair.to_name().unwrap()
        )),
        must_lex_and_parse_sc("fst p = casePair p k"),
        must_lex_and_parse_sc("snd p = casePair p k1"),
        must_lex_and_parse_sc(format!("cons = Pack{{{}, 2}}", CONS_TAG)),
        must_lex_and_parse_sc(format!("nil = Pack{{{}, 0}}", NIL_TAG)),
        must_lex_and_parse_sc(format!(
            "caseList l onNil onCons = {} l onNil onCons",
            PrimOpKind::MatchList.to_name().unwrap()
        )),
        must_lex_and_parse_sc("length l = caseList l lengthOnNil lengthOnCons"),
        must_lex_and_parse_sc("lengthOnNil = 0"),
        must_lex_and_parse_sc("lengthOnCons x xs = 1 + length xs"),
        must_lex_and_parse_sc("head l = caseList l panic k"),
        must_lex_and_parse_sc("tail l = caseList l panic k1"),
        must_lex_and_parse_sc(format!("panic = {}", PrimOpKind::Abort.to_name().unwrap())),
        must_lex_and_parse_sc(format!("unit = Pack{{{}, 0}}", UNIT_TAG)),
        must_lex_and_parse_sc(format!("stop = {}", PrimOpKind::Stop.to_name().unwrap())),
        must_lex_and_parse_sc(format!(
            "trace x y = {} x y",
            PrimOpKind::Print.to_name().unwrap()
        )),
        must_lex_and_parse_sc("traceId x = trace x x"),
        must_lex_and_parse_sc("traceList l = seq (_traceList l) l"),
        must_lex_and_parse_sc("_traceList l = caseList l unit _traceListOnCons"),
        must_lex_and_parse_sc("_traceListOnCons head tail = trace head (_traceList tail)"),
        must_lex_and_parse_sc(format!(
            "seq x y = {} x y",
            PrimOpKind::Seq.to_name().unwrap()
        )),
    ]
}
