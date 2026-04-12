fn main() -> Result<(), String> {
    let p = toy::parse_program(
        "
        id x = x;
        compose f g x = f (g x);
        twice f = compose f f;
        main = twice twice id 3
    ",
    )?;
    let mut machine = toy::template_instantiation::Machine::new(p)?;
    machine.eval()?;
    println!("{:#?}", machine);
    println!("{:#?}", machine.stats);
    println!("{:#?}", machine.peak_node().borrow());
    Ok(())
}
