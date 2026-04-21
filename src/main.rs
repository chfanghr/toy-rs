use std::{env::args, fs::read_to_string};

const DEMO_PROGRAM: &'static str = "
        main = 1 == neg 1
    ";

fn main() -> Result<(), String> {
    let args = args().collect::<Vec<_>>();
    let source_file_content = args
        .get(1)
        .map(|p| read_to_string(p).map_err(|err| err.to_string()))
        .transpose()?;
    let source_file_content = source_file_content
        .as_ref()
        .map_or(DEMO_PROGRAM, |c| c.as_str());
    let p = toy::program_from_text(source_file_content)?;
    let mut machine = toy::template_instantiation::Machine::new(p);
    machine.eval(None)?;
    eprintln!("{:#?}", machine);
    println!("{:#?}", machine.peak_node().borrow());
    Ok(())
}
