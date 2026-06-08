use std::{collections::BTreeMap, iter, rc::Rc};

use stacksafe::stacksafe;

use crate::parser::{
    PRIM_ADD_NAME, PRIM_BOOLEAN_AND_NAME, PRIM_BOOLEAN_OR_NAME, PRIM_DIV_NAME, PRIM_EQ_NAME,
    PRIM_GE_NAME, PRIM_GT_NAME, PRIM_LE_NAME, PRIM_LT_NAME, PRIM_MUL_NAME, PRIM_NE_NAME,
    PRIM_SUB_NAME, ast,
};

use super::types::*;

type Env = Rc<BTreeMap<ast::Name, usize>>;

pub(super) fn p(p: &ast::Program<ast::Name>) -> CompiledProgram {
    let o: BTreeMap<ast::Name, (usize, Code)> =
        p.0.iter()
            .map(|s| (s.name.clone(), (s.arguments.len(), Code::new(sc(&s)))))
            .collect();
    // TODO: better error message;
    assert_eq!(p.0.len(), o.len());
    CompiledProgram::new(o)
}

pub(super) fn sc(sc: &ast::SuperCombinator<ast::Name>) -> Vec<Instruction> {
    let env = sc
        .arguments
        .iter()
        .cloned()
        .enumerate()
        .map(|(o, n)| (n, o))
        .collect::<BTreeMap<ast::Name, usize>>();

    assert_eq!(env.len(), sc.arguments.len());

    r(env.len(), &sc.body, Rc::new(env))
}

fn r(d: usize, expr: &ast::Expr<ast::Name>, env: Env) -> Vec<Instruction> {
    e(expr, env)
        .into_iter()
        .chain(iter::once(Instruction::Update(d)))
        .chain((d != 0).then_some(Instruction::Pop(d)))
        .chain(iter::once(Instruction::Unwind))
        .collect()
}

#[stacksafe]
fn e(expr: &ast::Expr<ast::Name>, env: Env) -> Vec<Instruction> {
    match expr {
        ast::Expr::Num(i) => Some(compile_num(i)),
        ast::Expr::Ap(_) => e_ap(expr, Rc::clone(&env)),
        ast::Expr::Let(l) => Some(mk_compile_let(c, e)(&l, Rc::clone(&env))),
        ast::Expr::IfThenElse(if_then_else) => Some(e_if_then_else(if_then_else, Rc::clone(&env))),
        _ => None,
    }
    .unwrap_or_else(|| e_fallback(expr, env))
}

fn e_fallback(expr: &ast::Expr<ast::Name>, env: Env) -> Vec<Instruction> {
    c(expr, env)
        .into_iter()
        .chain(iter::once(Instruction::Eval))
        .collect()
}

fn e_ap(ap: &ast::Expr<ast::Name>, env: Env) -> Option<Vec<Instruction>> {
    try {
        let [f, a_1, a_2, a_3] = match_n_ap(4, ap)?;
        e_ap_3(f, a_1, a_2, a_3, Rc::clone(&env))?
    }
    .or_else(|| try {
        let [f, a_1, a_2] = match_n_ap(3, ap)?;
        e_ap_2(f, a_1, a_2, env)?
    })
}

fn match_n_ap<'a, O: TryFrom<Vec<&'a ast::Expr<ast::Name>>>>(
    n: usize,
    e: &'a ast::Expr<ast::Name>,
) -> Option<O> {
    let mut e = e;
    let mut res = vec![];

    while res.len() < n {
        if let ast::Expr::Ap(a) = e {
            res.push(&a.r);
            e = &a.l;
        } else {
            res.push(e);
            break;
        }
    }

    res.reverse();
    O::try_from(res).ok()
}

fn extract_var_expr(e: &ast::Expr<ast::Name>) -> Option<&ast::Name> {
    match e {
        ast::Expr::Var(n) => Some(n),
        _ => None,
    }
}

fn e_ap_2(
    f: &ast::Expr<ast::Name>,
    a_1: &ast::Expr<ast::Name>,
    a_2: &ast::Expr<ast::Name>,
    env: Env,
) -> Option<Vec<Instruction>> {
    let f = extract_var_expr(f)?;
    let ap_instr = match f.0.as_str() {
        PRIM_ADD_NAME => Some(Instruction::Add),
        PRIM_SUB_NAME => Some(Instruction::Sub),
        PRIM_EQ_NAME => Some(Instruction::Eq),
        PRIM_GE_NAME => Some(Instruction::Ge),
        PRIM_NE_NAME => Some(Instruction::Ne),
        PRIM_GT_NAME => Some(Instruction::Gt),
        PRIM_LE_NAME => Some(Instruction::Le),
        PRIM_LT_NAME => Some(Instruction::Lt),
        PRIM_BOOLEAN_AND_NAME => Some(Instruction::BooleanAnd),
        PRIM_BOOLEAN_OR_NAME => Some(Instruction::BooleanOr),
        PRIM_DIV_NAME => Some(Instruction::Div),
        PRIM_MUL_NAME => Some(Instruction::Mul),
        _ => None,
    }?;

    Some(mk_compile_ap_raw(e, ap_instr)(a_1, a_2, env))
}

fn e_ap_3(
    f: &ast::Expr<ast::Name>,
    a_1: &ast::Expr<ast::Name>,
    a_2: &ast::Expr<ast::Name>,
    a_3: &ast::Expr<ast::Name>,
    env: Env,
) -> Option<Vec<Instruction>> {
    let f = extract_var_expr(f)?;
    match f.0.as_str() {
        "_prim_if" => {
            let pred_code = e(a_1, Rc::clone(&env));
            let then_branch_code = e(a_2, Rc::clone(&env));
            let else_branch_code = e(a_3, env);

            let res = pred_code
                .into_iter()
                .chain(iter::once(Instruction::new_branch(
                    then_branch_code,
                    else_branch_code,
                )))
                .collect();
            Some(res)
        }
        _ => None,
    }
}

fn e_if_then_else(if_then_else: &ast::IfThenElse<ast::Name>, env: Env) -> Vec<Instruction> {
    let pred_code = e(&if_then_else.pred, Rc::clone(&env));
    let then_branch_code = e(&if_then_else.then_branch, Rc::clone(&env));
    let else_branch_code = e(&if_then_else.else_branch, env);

    let res = pred_code
        .into_iter()
        .chain(iter::once(Instruction::new_branch(
            then_branch_code,
            else_branch_code,
        )))
        .collect();

    res
}

pub const PRIM_LAZY_IF: &str = "_prim_if";

fn mk_ap_chain(es: Vec<ast::Expr<ast::Name>>) -> ast::Expr<ast::Name> {
    es.into_iter()
        .reduce(|acc, a| ast::Expr::Ap(Box::new(ast::Application { l: acc, r: a })))
        .unwrap()
}

#[stacksafe]
fn c(expr: &ast::Expr<ast::Name>, env: Env) -> Vec<Instruction> {
    match expr {
        ast::Expr::Var(name) => vec![
            env.get(name)
                .map(|x| Instruction::Push(*x))
                .unwrap_or(Instruction::PushGlobal(name.clone())),
        ],
        ast::Expr::Num(i) => compile_num(i),
        ast::Expr::Ap(ap) => mk_compile_ap_raw(c, Instruction::MkAp)(&ap.l, &ap.r, env),
        ast::Expr::Let(l) => mk_compile_let(c, c)(&l, env),
        ast::Expr::IfThenElse(if_then_else) => c(
            &mk_ap_chain(vec![
                ast::Expr::Var(ast::Name::new(PRIM_LAZY_IF)),
                if_then_else.pred.clone(),
                if_then_else.then_branch.clone(),
                if_then_else.else_branch.clone(),
            ]),
            env,
        ),
        expr => todo!("cannot compile this expr: {:?}", expr),
    }
}

fn offset_env_by(env: Env, n: isize) -> Env {
    let mut env = env;
    let env_mut = Rc::make_mut(&mut env);
    env_mut
        .values_mut()
        .for_each(|x| *x = x.checked_add_signed(n).unwrap());
    env
}

fn compile_num(i: &ast::Integer) -> Vec<Instruction> {
    vec![Instruction::PushNum(i.0)]
}

fn mk_compile_ap_raw<F>(
    compile_lr: F,
    ap_instr: Instruction,
) -> impl FnOnce(&ast::Expr<ast::Name>, &ast::Expr<ast::Name>, Env) -> Vec<Instruction>
where
    F: Fn(&ast::Expr<ast::Name>, Env) -> Vec<Instruction>,
{
    move |l, r, env| {
        let r = compile_lr(r, Rc::clone(&env));

        let env = offset_env_by(env, 1);
        let l = compile_lr(l, env);

        r.into_iter().chain(l).chain(iter::once(ap_instr)).collect()
    }
}

fn mk_compile_let<F, G>(
    compile_def: F,
    compiler_body: G,
) -> impl FnOnce(&ast::Let<ast::Name>, Env) -> Vec<Instruction>
where
    F: Fn(&ast::Expr<ast::Name>, Env) -> Vec<Instruction>,
    G: FnOnce(&ast::Expr<ast::Name>, Env) -> Vec<Instruction>,
{
    move |l, env| {
        let n_defs = l.definitions.len();
        let (env, mut code) = if l.is_recursive {
            let mut env = offset_env_by(env, isize::try_from(n_defs).unwrap());
            let env_mut = Rc::make_mut(&mut env);
            let bs = l
                .definitions
                .iter()
                .rev()
                .enumerate()
                .scan(env_mut, |env, (idx, b)| {
                    env.insert(b.binder.clone(), idx);
                    Some((idx, &b.body))
                })
                .collect::<Vec<_>>();
            let env = env;
            let code = iter::once(Instruction::Alloc(n_defs))
                .chain(bs.into_iter().flat_map(|(idx, expr)| {
                    compile_def(expr, Rc::clone(&env))
                        .into_iter()
                        .chain(iter::once(Instruction::Update(idx)))
                }))
                .collect();
            (env, code)
        } else {
            l.definitions
                .iter()
                .fold((env, vec![]), |(env, mut code), b| {
                    code.extend(compile_def(&b.body, Rc::clone(&env)));
                    let mut env = offset_env_by(env, 1);
                    Rc::make_mut(&mut env).insert(b.binder.clone(), 0);
                    (env, code)
                })
        };
        code.extend(compiler_body(&l.body, env));
        code.push(Instruction::Slide(n_defs));
        code
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::must_lex_and_parse_sc;

    use super::{Instruction::*, *};

    fn assert_instr_sequence_test(inp: &str, expected: Vec<Instruction>) {
        let sc_ast = must_lex_and_parse_sc(inp);
        let actual = sc(&sc_ast);
        assert_eq!(actual, expected, "compiling {}", inp)
    }

    #[test]
    fn sc_prim_ad() {
        assert_instr_sequence_test(
            "_prim_add x y = x + y",
            // stack layout: [top]  x y  (Ap _prim_add x) [bottom]
            vec![
                Push(1), // y (rhs)
                Eval,    // y to WHNF
                Push(1), // x (lhs)
                Eval,    // x to WHNF
                Add,
                Update(2), // Override the (Ap _prim_add x) node
                Pop(2),
                Unwind,
            ],
        );
    }

    #[test]
    fn sc_prim_if_then_else() {
        assert_instr_sequence_test(
            "_prim_if_then_else pred thenBranch elseBranch = 
                        if pred then thenBranch else elseBranch 
                ",
            vec![
                Push(0), // predicate
                Eval,    // predicate to WHNF
                Instruction::new_branch(vec![Push(1), Eval], vec![Push(2), Eval]),
                Update(3),
                Pop(3),
                Unwind,
            ],
        )
    }

    #[test]
    fn one() {
        assert_instr_sequence_test("one = 1", vec![PushNum(1), Update(0), Unwind]);
    }

    #[test]
    fn two() {
        assert_instr_sequence_test(
            "two = 1 + 1",
            vec![PushNum(1), PushNum(1), Add, Update(0), Unwind],
        );
    }

    #[test]
    fn three() {
        assert_instr_sequence_test(
            "three = 1 + 1 + 1",
            vec![
                PushNum(1),
                PushNum(1),
                PushNum(1),
                Add,
                Add,
                Update(0),
                Unwind,
            ],
        );
    }

    #[test]
    fn four() {
        assert_instr_sequence_test(
            "four = 1 + i 1 + 3 - 1",
            vec![
                PushNum(1),
                PushNum(3),
                PushNum(1),
                PushGlobal(ast::Name::new("i")),
                MkAp,
                Eval,
                PushNum(1),
                Add,
                Add,
                Sub,
                Update(0),
                Unwind,
            ],
        );
    }

    #[test]
    fn nested_let_binds() {
        assert_instr_sequence_test(
            "nestedLetBinds f = 
                    letrec x = f x in 
                        let y = x in 
                            x + y",
            vec![
                Alloc(1),  // allocate x
                Push(0),   // uninitialized x
                Push(2),   // f
                MkAp,      // f x
                Update(0), // initialze x
                Push(0),   // y = x
                Push(0),   // y (rhs of x + y)
                Eval,      // y to WHNF
                Push(2),   // x (lhs of x + y)
                Eval,      // x to WHNF
                Add,       // (+)
                Slide(1),  // drop the inner let clause
                Slide(1),  // drop the outer let clause
                Update(1), // Caller's Ap nestedLetBinds _ node
                Pop(1),
                Unwind,
            ],
        )
    }

    #[test]
    fn if_then_else() {
        assert_instr_sequence_test(
            "ifThenElse = 
                let bind1=if true && false then 42 + 1 else 69;
                    bind2=if true || false false then 42 else 69
                    in if bind1 > bind2 then 0 else 1
            ",
            vec![
                PushNum(69),
                PushNum(1),
                PushNum(42),
                PushGlobal(ast::Name::new("_prim_add")),
                MkAp,
                MkAp,
                PushGlobal(ast::Name::new("false")),
                PushGlobal(ast::Name::new("true")),
                PushGlobal(ast::Name::new("_prim_boolean_and")),
                MkAp,
                MkAp,
                PushGlobal(ast::Name::new("_prim_if")),
                MkAp,
                MkAp,
                MkAp,
                PushNum(69),
                PushNum(42),
                PushGlobal(ast::Name::new("false")),
                PushGlobal(ast::Name::new("false")),
                MkAp,
                PushGlobal(ast::Name::new("true")),
                PushGlobal(ast::Name::new("_prim_boolean_or")),
                MkAp,
                MkAp,
                PushGlobal(ast::Name::new("_prim_if")),
                MkAp,
                MkAp,
                MkAp,
                Push(0),
                Eval,
                Push(2),
                Eval,
                Gt,
                Instruction::new_branch(vec![PushNum(0)], vec![PushNum(1)]),
                Slide(2),
                Update(0),
                Unwind,
            ],
        );
    }
}
