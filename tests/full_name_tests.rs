use rust_bible_struct::BibleBook;

#[test]
fn full_name_representative_books() {
    assert_eq!(BibleBook::Genesis.full_name(), "Genesis");
    assert_eq!(BibleBook::Psalms.full_name(), "Psalms");
    assert_eq!(BibleBook::Revelation.full_name(), "Revelation");
    assert_eq!(BibleBook::Tobit.full_name(), "Tobit");
    assert_eq!(
        BibleBook::DanielBelAndTheDragon.full_name(),
        "Daniel (Bel and the Dragon)"
    );
    assert_eq!(BibleBook::FourthMaccabees.full_name(), "4 Maccabees");
}

#[test]
fn full_name_all_books() {
    use rust_bible_struct::BibleBook::*;

    const EXPECTED: &[(BibleBook, &str)] = &[
        (Genesis, "Genesis"),
        (Exodus, "Exodus"),
        (Leviticus, "Leviticus"),
        (Numbers, "Numbers"),
        (Deuteronomy, "Deuteronomy"),
        (Joshua, "Joshua"),
        (Judges, "Judges"),
        (Ruth, "Ruth"),
        (FirstSamuel, "1 Samuel"),
        (SecondSamuel, "2 Samuel"),
        (FirstKings, "1 Kings"),
        (SecondKings, "2 Kings"),
        (FirstChronicles, "1 Chronicles"),
        (SecondChronicles, "2 Chronicles"),
        (Ezra, "Ezra"),
        (Nehemiah, "Nehemiah"),
        (Esther, "Esther"),
        (Job, "Job"),
        (Psalms, "Psalms"),
        (Proverbs, "Proverbs"),
        (Ecclesiastes, "Ecclesiastes"),
        (SongOfSolomon, "Song of Solomon"),
        (Isaiah, "Isaiah"),
        (Jeremiah, "Jeremiah"),
        (Lamentations, "Lamentations"),
        (Ezekiel, "Ezekiel"),
        (Daniel, "Daniel"),
        (Hosea, "Hosea"),
        (Joel, "Joel"),
        (Amos, "Amos"),
        (Obadiah, "Obadiah"),
        (Jonah, "Jonah"),
        (Micah, "Micah"),
        (Nahum, "Nahum"),
        (Habakkuk, "Habakkuk"),
        (Zephaniah, "Zephaniah"),
        (Haggai, "Haggai"),
        (Zechariah, "Zechariah"),
        (Malachi, "Malachi"),
        (Matthew, "Matthew"),
        (Mark, "Mark"),
        (Luke, "Luke"),
        (John, "John"),
        (Acts, "Acts"),
        (Romans, "Romans"),
        (FirstCorinthians, "1 Corinthians"),
        (SecondCorinthians, "2 Corinthians"),
        (Galatians, "Galatians"),
        (Ephesians, "Ephesians"),
        (Philippians, "Philippians"),
        (Colossians, "Colossians"),
        (FirstThessalonians, "1 Thessalonians"),
        (SecondThessalonians, "2 Thessalonians"),
        (FirstTimothy, "1 Timothy"),
        (SecondTimothy, "2 Timothy"),
        (Titus, "Titus"),
        (Philemon, "Philemon"),
        (Hebrews, "Hebrews"),
        (James, "James"),
        (FirstPeter, "1 Peter"),
        (SecondPeter, "2 Peter"),
        (FirstJohn, "1 John"),
        (SecondJohn, "2 John"),
        (ThirdJohn, "3 John"),
        (Jude, "Jude"),
        (Revelation, "Revelation"),
        (Tobit, "Tobit"),
        (Judith, "Judith"),
        (Wisdom, "Wisdom"),
        (Sirach, "Sirach"),
        (Baruch, "Baruch"),
        (FirstMaccabees, "1 Maccabees"),
        (SecondMaccabees, "2 Maccabees"),
        (EstherAdditions, "Esther (Greek)"),
        (DanielSongOfThree, "Daniel (Song of Three)"),
        (DanielSusanna, "Daniel (Susanna)"),
        (DanielBelAndTheDragon, "Daniel (Bel and the Dragon)"),
        (FirstEsdras, "1 Esdras"),
        (SecondEsdras, "2 Esdras"),
        (PrayerOfManasseh, "Prayer of Manasseh"),
        (Psalm151, "Psalm 151"),
        (ThirdMaccabees, "3 Maccabees"),
        (FourthMaccabees, "4 Maccabees"),
    ];

    assert_eq!(EXPECTED.len(), 83); // ensure all variants covered
    for (book, expected) in EXPECTED {
        assert_eq!(book.full_name(), *expected);
    }
}
