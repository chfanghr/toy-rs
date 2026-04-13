pub mod lexer;
pub mod parser;
pub mod template_instantiation;

#[macro_use]
extern crate custom_derive;
#[macro_use]
extern crate enum_derive;

pub fn parse_program(i: &str) -> Result<parser::ast::Program<parser::ast::Name>, String> {
    use nom::Parser;

    let (_, tokens) = lexer::token_vec.parse(i).map_err(|err| err.to_string())?;
    let tokens = lexer::tokens::Tokens::new(&tokens);
    let (_, program) = parser::program
        .parse(tokens)
        .map_err(|err| err.to_string())?;

    Ok(program)
}
