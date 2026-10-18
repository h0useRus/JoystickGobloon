//! Re-saves a Gremlin profile through Joystick Gobloon: `cargo run -p gob-gremlin --example resave -- in.xml out.xml`
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, input, output] = args.as_slice() else {
        eprintln!("usage: resave <in.xml> <out.xml>");
        std::process::exit(2);
    };
    let profile = gob_gremlin::load(input).unwrap_or_else(|e| panic!("{input}: {e}"));
    gob_gremlin::save(output, &profile).unwrap_or_else(|e| panic!("{output}: {e}"));
    println!("{:#?}", gob_gremlin::report(&profile).warnings);
}
