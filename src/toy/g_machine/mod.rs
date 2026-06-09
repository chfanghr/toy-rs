mod compiler;
mod machine;
mod prelude;
mod types;

#[cfg(test)]
mod tests {
    use chumsky::Parser;

    use crate::{
        g_machine::{
            compiler::p,
            machine::{Machine, MachineIter},
            prelude::link_with_prelude,
        },
        lexer::token_vec,
        parser::{ast, parser},
    };

    #[test]
    fn t() {
        let entry_point = ast::Name::new("main");
        let program = "i x = x; main = i 42";
        let tokens = token_vec().parse(program).unwrap();
        let ast = parser().parse(&tokens).unwrap();
        let compiled = p(&ast);
        // let compiled = link_with_prelude(compiled);
        println!("compiled: {:?}", compiled);
        let machine = Machine::new(compiled, entry_point);
        MachineIter::new(machine).for_each(|r| {
            println!("{:?}\n", r);
            r.unwrap();
        });
    }
}
