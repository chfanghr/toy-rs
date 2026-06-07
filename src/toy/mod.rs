#![feature(try_blocks)]
#![feature(trait_alias)]

pub mod g_machine;
pub mod lexer;
pub mod parser;
pub mod template_instantiation;
pub mod utils;

#[macro_use]
extern crate custom_derive;
#[macro_use]
extern crate enum_derive;

#[macro_use]
extern crate static_assertions;

use anyhow::{Result, anyhow};

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
