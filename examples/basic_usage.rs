use bible_io::{Bible, BibleBook};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file_path = "tests/fixtures/en_kjv.json";

    let bible = Bible::new_from_json(file_path)?;

    match bible.get_verse(BibleBook::Genesis, 1, 1) {
        Ok(verse) => println!("{}", verse),
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}
