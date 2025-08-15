use rust_bible_struct::{Bible, BibleBook};

fn main() {
    let file_path = "tests/fixtures/en_kjv.json";

    let bible: Bible = Bible::new_from_json(file_path);

    let verse = bible.get_verse(BibleBook::Genesis, 1, 1).unwrap();

    println!("{}", verse);
}
