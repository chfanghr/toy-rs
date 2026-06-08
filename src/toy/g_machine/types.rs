use std::{collections::BTreeMap, rc::Rc};

use stacksafe::StackSafe;

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
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Code(pub(super) Rc<Vec<Instruction>>);

impl Code {
    pub(super) fn new(instructions: Vec<Instruction>) -> Self {
        Self(Rc::new(instructions))
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CompiledProgram(pub(super) BTreeMap<ast::Name, (usize, Code)>);

impl CompiledProgram {
    pub fn new(btree_map: BTreeMap<ast::Name, (usize, Code)>) -> Self {
        Self(btree_map)
    }
}
