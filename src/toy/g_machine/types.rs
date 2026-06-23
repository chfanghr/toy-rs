use std::{collections::BTreeMap, rc::Rc};

use intmap::IntMap;
use pretty::{DocAllocator, DocBuilder};
use stacksafe::{StackSafe, stacksafe};

use crate::parser::ast;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Instruction {
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
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    BooleanAnd,
    BooleanOr,

    // TODO: flatten this?
    Branch(StackSafe<Code>, StackSafe<Code>),

    Pack(u64 /* tag */, usize /* number of fields */),
    PushPack(u64, usize),
    CaseJump(IntMap<u64 /* tag */, StackSafe<Code>> /* branches */),
    Split(usize),

    Abort,
}

impl Instruction {
    pub(super) fn new_branch(
        then_branch_code: Vec<Instruction>,
        else_branch_code: Vec<Instruction>,
    ) -> Self {
        Self::Branch(
            StackSafe::new(Code::new(then_branch_code)),
            StackSafe::new(Code::new(else_branch_code)),
        )
    }

    #[stacksafe]
    fn pp_branch<'b, D, A>(a: &'b D, b: &'b StackSafe<Code>) -> DocBuilder<'b, D, A>
    where
        D: DocAllocator<'b, A>,
        D::Doc: Clone,
        A: Clone,
    {
        b.pp(a)
    }

    #[stacksafe]
    pub fn pp_multi<'b, D, A, I>(a: &'b D, i: I, limit: Option<usize>) -> DocBuilder<'b, D, A>
    where
        D: DocAllocator<'b, A>,
        D::Doc: Clone,
        A: Clone,
        I: IntoIterator<Item = &'b Self>,
    {
        a.concat([
            a.text("{"),
            i.into_iter()
                .enumerate()
                .map_while(|(i, x)| match (i, limit) {
                    (_, None) => Some(a.hardline().append(x.pp(a))),
                    (i, Some(limit)) if i < limit => Some(a.hardline().append(x.pp(a))),
                    (i, Some(limit)) if i == limit => Some(a.text("...")),
                    _ => None,
                })
                .fold(a.nil(), |acc, x| acc.append(x))
                .group()
                .nest(2),
            a.hardline(),
            a.text("}"),
        ])
    }

    #[stacksafe]
    pub fn pp<'b, D, A>(&'b self, a: &'b D) -> DocBuilder<'b, D, A>
    where
        D: DocAllocator<'b, A>,
        D::Doc: Clone,
        A: Clone,
    {
        match self {
            Instruction::Unwind => a.text("Unwind"),
            Instruction::PushGlobal(name) => {
                a.text("PushGlobal").append(a.space()).append(name.pp(a))
            }
            Instruction::PushNum(n) => a.concat([a.text("PushNum"), a.space(), a.as_string(n)]),
            Instruction::Push(n) => a.concat([a.text("Push"), a.space(), a.as_string(n)]),
            Instruction::MkAp => a.text("MkAp"),
            Instruction::Update(n) => a.concat([a.text("Update"), a.space(), a.as_string(n)]),
            Instruction::Pop(n) => a.concat([a.text("Pop"), a.space(), a.as_string(n)]),
            Instruction::Alloc(n) => a.concat([a.text("Alloc"), a.space(), a.as_string(n)]),
            Instruction::Slide(n) => a.concat([a.text("Slide"), a.space(), a.as_string(n)]),
            Instruction::Eval => a.text("Eval"),
            Instruction::Add => a.text("Add"),
            Instruction::Sub => a.text("Sub"),
            Instruction::Mul => a.text("Mul"),
            Instruction::Div => a.text("Div"),
            Instruction::Eq => a.text("Eq"),
            Instruction::Ne => a.text("Ne"),
            Instruction::Gt => a.text("Gt"),
            Instruction::Ge => a.text("Ge"),
            Instruction::Lt => a.text("Lt"),
            Instruction::Le => a.text("Le"),
            Instruction::BooleanAnd => a.text("BooleanAnd"),
            Instruction::BooleanOr => a.text("BooleanOr"),
            Instruction::Branch(then_branch, else_branch) => a.text("Branch").append(
                a.concat([
                    a.line(),
                    a.text("then:").append(a.space()).append(then_branch.pp(a)),
                    a.line(),
                    a.text("else:").append(a.space()).append(else_branch.pp(a)),
                ])
                .group()
                .nest(2),
            ),
            Instruction::Pack(tag, n_fields) => a.concat([
                a.text("Pack"),
                a.space(),
                a.as_string(tag),
                a.space(),
                a.as_string(n_fields),
            ]),
            Instruction::PushPack(tag, n_fields) => a.concat([
                a.text("PushPack"),
                a.space(),
                a.as_string(tag),
                a.space(),
                a.as_string(n_fields),
            ]),
            Instruction::CaseJump(branches) => a.concat([
                a.text("CaseJump"),
                a.space(),
                a.text("["),
                a.hardline(),
                branches
                    .iter()
                    .fold(a.nil(), |acc, (tag, code)| {
                        a.concat([
                            acc,
                            a.hardline(),
                            a.as_string(tag),
                            a.space(),
                            a.text("->"),
                            a.space(),
                            code.pp(a),
                        ])
                    })
                    .group()
                    .nest(2),
                a.hardline(),
                a.text("]"),
            ]),
            Instruction::Split(n_fields) => {
                a.concat([a.text("Split"), a.space(), a.as_string(n_fields)])
            }
            Instruction::Abort => a.text("Stop"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Code(pub(super) Rc<Vec<Instruction>>);

impl Code {
    pub(super) fn new(instructions: Vec<Instruction>) -> Self {
        Self(Rc::new(instructions))
    }

    #[stacksafe]
    pub fn pp<'b, D, A>(&'b self, a: &'b D) -> DocBuilder<'b, D, A>
    where
        D: DocAllocator<'b, A>,
        D::Doc: Clone,
        A: Clone,
    {
        Instruction::pp_multi(a, self.0.iter(), None)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CompiledProgram(pub(super) BTreeMap<ast::Name, (usize, Code)>);

impl CompiledProgram {
    pub fn new(btree_map: BTreeMap<ast::Name, (usize, Code)>) -> Self {
        Self(btree_map)
    }
}

#[cfg(test)]
mod tests {
    use crate::{g_machine::compiler::sc, parser::must_lex_and_parse_sc};

    use super::*;

    use pretty::Arena;

    fn pp_compiled_sc(inp: &str) {
        let sc_ast = must_lex_and_parse_sc(inp);
        let instrs = sc(&sc_ast);
        let code = Code::new(instrs);

        let arena = Arena::<()>::new();

        println!("\n{}", code.pp(&arena).pretty(80));
    }

    #[test]
    fn test() {
        pp_compiled_sc("main = 1");
        pp_compiled_sc(
            "_prim_if pred then_branch else_branch = if pred then then_branch else else_branch",
        );
        pp_compiled_sc("fix f = letrec x = f x in x");
    }
}
