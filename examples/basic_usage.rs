use rust_bible_struct::{Bible, BibleBook};

fn main() {
    let file_path = "tests/fixtures/en_kjv.json";

    let bible: Bible = Bible::new_from_json(file_path);

    match bible.get_verse(BibleBook::Genesis, 1, 1) {
        Ok(verse) => println!("{}", verse),
        Err(e) => eprintln!("Error: {}", e),
    }
}
