use crate::{g_machine::types::*, utils::stack::Stack};

// #[derive(Debug, Clone, PartialEq, Eq)]
// enum Node {
//     Num(i64),
//     Ap(Addr, Addr),
//     Global(usize, Rc<Code>),
//     Indirect(Addr),
// }

struct Machine {
  code: Code,
  instr_ptr: usize,
  stack: Stack<Node>
}