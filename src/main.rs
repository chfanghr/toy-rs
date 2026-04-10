fn main() -> Result<(), String> {
    let p = toy::parse_program("main = let x = 69; y = 42 in s k i x y")?;
    let mut machine = toy::template_instantiation::Machine::new(p)?;
    machine.eval()?;
    // println!("{:#?}", machine);
    println!("{:#?}", machine.stats);
    println!("{:#?}", machine.peak_node().borrow());
    Ok(())
}
