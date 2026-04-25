pub mod lexer;
pub mod parser;
pub mod template_instantiation;

#[macro_use]
extern crate custom_derive;
#[macro_use]
extern crate enum_derive;

use anyhow::{anyhow, Result};

pub fn program_from_text<S: AsRef<str>>(i: S) -> Result<parser::ast::Program<parser::ast::Name>> {
    use chumsky::Parser;

    let tokens = lexer::token_vec()
        .parse(i.as_ref())
        .into_result()
        .map_err(|err| anyhow!("lexer: {:?}", err))?;
    let program = parser::parser()
        .parse(&tokens)
        .into_result()
        .map_err(|err| anyhow!("parser: {:?}", err))?;
    Ok(program)
}
