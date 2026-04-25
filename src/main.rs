use anyhow::Result;
use clap::Parser;
use log::{debug, error, trace};
use std::{fs, io, path::PathBuf, process::exit};
use toy::parser::ast;

#[derive(Parser, Debug)]
struct Cli {
    #[arg(short, long, value_name = "FILE")]
    input: Option<PathBuf>,

    #[arg(short, long, default_value_t=String::from("main"), value_name = "ENRTY_FUNCTION")]
    entry: String,

    #[arg(short, long, action = clap::ArgAction::Count, default_value_t=0)]
    verbose: u8,
}

#[cfg(debug_assertions)]
fn fallback_source() -> Box<dyn io::Read> {
    Box::new("main = stop".as_bytes())
}

#[cfg(not(debug_assertions))]
fn fallback_source() -> Box<dyn io::Read> {
    let b: Box<dyn io::Read> = Box::new(io::stdin());
    b
}

fn main() {
    let cli = Cli::parse();

    stderrlog::new()
        .module(module_path!())
        .verbosity(cli.verbose as usize)
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();

    if let Err(err) = try_main(cli) {
        error!("ERROR: \n{:?}", err);
        exit(1)
    }
}

fn try_main(cli: Cli) -> Result<()> {
    let entry_point = ast::Name::new(cli.entry);

    let input_reader: Box<dyn io::Read> = cli.input.map_or_else(
        || Ok(fallback_source()),
        |f| -> io::Result<Box<dyn io::Read>> {
            debug!("reading source from: {}", f.to_string_lossy());
            let f = fs::File::open(f)?;
            let b: Box<dyn io::Read> = Box::new(f);
            Ok(b)
        },
    )?;
    let input_content = io::read_to_string(input_reader)?;

    debug!("parsing");
    let p = toy::program_from_text(input_content)?;
    debug!("done parsing");
    trace!("ast: {:#?}", p);

    debug!("constructing template instantiation machine");
    let mut machine = toy::template_instantiation::Machine::new(p);
    debug!("done constructing template instantiation machine");
    trace!("initial machine: {:#?}", machine);

    debug!("executing");
    machine.eval(&entry_point)?;
    debug!("done executing");
    trace!("final machine: {:#?}", machine);

    {
        let entry_addr = *machine.globals.lookup(&entry_point).unwrap();
        let entry_addr = machine.follow_indirect(entry_addr);
        let entry_node = machine.heap.access(entry_addr).unwrap().borrow();
        println!("{:?}", entry_node)
    }

    Ok(())
}
