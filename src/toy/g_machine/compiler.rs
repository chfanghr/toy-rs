use std::{
    collections::{BTreeMap, LinkedList, VecDeque},
    iter::{self, once},
    rc::Rc,
};

use chumsky::{container::Container, primitive::todo};
use itertools::Either;

use crate::{
    g_machine::compiler,
    parser::{PRIM_ADD_NAME, PRIM_SUB_NAME, ast},
};

#[derive(Debug, PartialEq, Eq, Clone)]
enum Instruction {
    Unwind,
    PushGlobal(ast::Name),
    PushNum(i64),
    Push(usize),
    MkAp,
    Update(usize),
    Pop(usize),
    Alloc(usize),
    Slide(usize),
    Eval,
    Add,
    Branch(Box<Code>, Box<Code>),
}

type Code = Vec<Instruction>;

type Assoc<K, V> = BTreeMap<K, V>;

type Env = Rc<Assoc<ast::Name, usize>>;

// #[derive(Debug, Clone)]
// enum CompilationTodo {
//     ToCompile(ast::Expr<ast::Name>, Env),
//     Done(Instruction),
// }

// #[derive(Debug, Clone)]
// struct ExpressionCompiler {
//     todo_stack: VecDeque<CompilationTodo>,
//     code_output: Code,
// }

// impl ExpressionCompiler {
//     fn push_todo(&mut self, todo: CompilationTodo) {
//         self.todo_stack.push_back(todo);
//     }

//     fn pop_todo(&mut self) -> Option<CompilationTodo> {
//         self.todo_stack.pop_back()
//     }

//     fn new(expr: ast::Expr<ast::Name>, env: Env) -> Self {
//         let mut res = Self {
//             todo_stack: VecDeque::new(),
//             code_output: Code::new(),
//         };

//         res.push_todo(CompilationTodo::ToCompile(expr, env));

//         res
//     }

// }

// fn is_prim(name: &ast::Name) -> bool {
//     todo!()
// }

// fn e_fallback(expr: ast::Expr<ast::Name>, env: Env) -> Vec<Instruction> {
//     let mut res = c(expr, env);
//     res.push(Instruction::Eval);
//     res
// }

fn sc(sc: &ast::SuperCombinator<ast::Name>) -> Code {
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

fn r(d: usize, expr: &ast::Expr<ast::Name>, env: Env) -> Code {
    e(expr, env)
        .into_iter()
        .chain(iter::once(Instruction::Update(d)))
        .chain(iter::once(Instruction::Pop(d)))
        .chain(iter::once(Instruction::Unwind))
        .collect()
}

fn e(expr: &ast::Expr<ast::Name>, env: Env) -> Code {
    match expr {
        ast::Expr::Num(i) => Some(compile_num(i)),
        ast::Expr::Ap(_) => e_ap(expr, env.clone()),
        ast::Expr::Let(l) => Some(mk_compile_let(c, e)(&l, env.clone())),
        _ => None,
    }
    .unwrap_or_else(|| e_fallback(expr, env))
}

fn e_fallback(expr: &ast::Expr<ast::Name>, env: Env) -> Code {
    c(expr, env)
        .into_iter()
        .chain(iter::once(Instruction::Eval))
        .collect()
}

fn e_ap(ap: &ast::Expr<ast::Name>, env: Env) -> Option<Code> {
    try {
        let [f, a_1, a_2, a_3] = match_n_ap(4, ap)?;
        e_ap_3(f, a_1, a_2, a_3, env.clone())?
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
) -> Option<Code> {
    let f = extract_var_expr(f)?;
    let ap_instr = match f.0.as_str() {
        PRIM_ADD_NAME => Some(Instruction::Add),
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
) -> Option<Code> {
    let f = extract_var_expr(f)?;
    match f.0.as_str() {
        "_prim_if" => {
            let pred_code = e(a_1, env.clone());
            let then_branch_code = e(a_2, env.clone());
            let else_branch_code = e(a_3, env);

            let res = pred_code
                .into_iter()
                .chain(iter::once(Instruction::Branch(
                    Box::new(then_branch_code),
                    Box::new(else_branch_code),
                )))
                .collect();
            Some(res)
        }
        _ => None,
    }
}

fn c(expr: &ast::Expr<ast::Name>, env: Env) -> Code {
    match expr {
        ast::Expr::Var(name) => vec![
            env.get(name)
                .map(|x| Instruction::Push(*x))
                .unwrap_or(Instruction::PushGlobal(name.clone())),
        ],
        ast::Expr::Num(i) => compile_num(i),
        ast::Expr::Ap(ap) => mk_compile_ap_raw(c, Instruction::MkAp)(&ap.l, &ap.r, env),
        ast::Expr::Let(l) => mk_compile_let(c, c)(&l, env),
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

fn compile_num(i: &ast::Integer) -> Code {
    vec![Instruction::PushNum(i.0)]
}

fn mk_compile_ap_raw<F>(
    compile_lr: F,
    ap_instr: Instruction,
) -> impl FnOnce(&ast::Expr<ast::Name>, &ast::Expr<ast::Name>, Env) -> Code
where
    F: Fn(&ast::Expr<ast::Name>, Env) -> Code,
{
    move |l, r, env| {
        let r = compile_lr(r, env.clone());

        let env = offset_env_by(env, 1);
        let l = compile_lr(l, env);

        r.into_iter().chain(l).chain(iter::once(ap_instr)).collect()
    }
}

fn mk_compile_let<F, G>(
    compile_def: F,
    compiler_body: G,
) -> impl FnOnce(&ast::Let<ast::Name>, Env) -> Code
where
    F: Fn(&ast::Expr<ast::Name>, Env) -> Code,
    G: FnOnce(&ast::Expr<ast::Name>, Env) -> Code,
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
                    compile_def(expr, env.clone())
                        .into_iter()
                        .chain(iter::once(Instruction::Update(idx)))
                }))
                .collect();
            (env, code)
        } else {
            l.definitions
                .iter()
                .fold((env, vec![]), |(env, mut code), b| {
                    code.extend(compile_def(&b.body, env.clone()));
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
mod test {
    use crate::parser::must_lex_and_parse_sc;

    use super::*;

    #[test]
    fn tt() {
        t("two = 1 + 1");
        t("three = 1 + 1 + 1");
        t("four = 1 + i 1 + 1 + 1");
        t("addOne x = letrec y = _prim_if true x x in y + 1");
    }

    fn t(c: &str) {
        let ast = must_lex_and_parse_sc(c);
        println!("{:?}", ast);
        let code = sc(&ast);
        println!("{:?}", code);
    }
}
