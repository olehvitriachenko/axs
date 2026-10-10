use axs::{parse, stringify};
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string("profile.axs")?;

    let data = parse(&source)?;
    let serialized = stringify(&data)?;

    println!("=== PARSED ===\n{data:#?}");
    println!("\n=== SERIALIZED ===\n{serialized}");

    assert_eq!(data, parse(&serialized)?);
    println!("Round-trip successful!");

    Ok(())
}
