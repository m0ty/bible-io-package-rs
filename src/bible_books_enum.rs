use std::str::FromStr;

/// Represents Bible books across Protestant (66), Catholic (Deuterocanon), and
/// Eastern Orthodox canons, using compact lowercase abbreviations suited for JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BibleBook {
    // --- Protestant (66) ---
    Genesis,             // "gn"
    Exodus,              // "ex"
    Leviticus,           // "lv"
    Numbers,             // "nm"
    Deuteronomy,         // "dt"
    Joshua,              // "js"
    Judges,              // "jud"
    Ruth,                // "rt"
    FirstSamuel,         // "1sm"
    SecondSamuel,        // "2sm"
    FirstKings,          // "1kgs"
    SecondKings,         // "2kgs"
    FirstChronicles,     // "1ch"
    SecondChronicles,    // "2ch"
    Ezra,                // "ezr"
    Nehemiah,            // "ne"
    Esther,              // "et"
    Job,                 // "job"
    Psalms,              // "ps"
    Proverbs,            // "prv"
    Ecclesiastes,        // "ec"
    SongOfSolomon,       // "so"
    Isaiah,              // "is"
    Jeremiah,            // "jr"
    Lamentations,        // "lm"
    Ezekiel,             // "ez"
    Daniel,              // "dn"
    Hosea,               // "ho"
    Joel,                // "jl"
    Amos,                // "am"
    Obadiah,             // "ob"
    Jonah,               // "jn"
    Micah,               // "mi"
    Nahum,               // "na"
    Habakkuk,            // "hk"
    Zephaniah,           // "zp"
    Haggai,              // "hg"
    Zechariah,           // "zc"
    Malachi,             // "ml"
    Matthew,             // "mt"
    Mark,                // "mk"
    Luke,                // "lk"
    John,                // "jo"
    Acts,                // "act"
    Romans,              // "rm"
    FirstCorinthians,    // "1co"
    SecondCorinthians,   // "2co"
    Galatians,           // "gl"
    Ephesians,           // "eph"
    Philippians,         // "ph"
    Colossians,          // "cl"
    FirstThessalonians,  // "1ts"
    SecondThessalonians, // "2ts"
    FirstTimothy,        // "1tm"
    SecondTimothy,       // "2tm"
    Titus,               // "tt"
    Philemon,            // "phm"
    Hebrews,             // "hb"
    James,               // "jm"
    FirstPeter,          // "1pe"
    SecondPeter,         // "2pe"
    FirstJohn,           // "1jo"
    SecondJohn,          // "2jo"
    ThirdJohn,           // "3jo"
    Jude,                // "jd"
    Revelation,          // "re",

    // --- Catholic Deuterocanon ---
    Tobit,                 // "tb"
    Judith,                // "jdt"
    Wisdom,                // "ws"    (a.k.a. Wisdom of Solomon)
    Sirach,                // "sir"   (a.k.a. Ecclesiasticus)
    Baruch,                // "bar"
    FirstMaccabees,        // "1mc"
    SecondMaccabees,       // "2mc"
    EstherAdditions,       // "etg"   (Greek additions to Esther)
    DanielSongOfThree,     // "dn3"   (Song of the Three Holy Children)
    DanielSusanna,         // "dns"   (Susanna)
    DanielBelAndTheDragon, // "dnb"   (Bel and the Dragon)

    // --- Eastern Orthodox Additions (Anagignoskomena) ---
    FirstEsdras,      // "1es"
    SecondEsdras,     // "2es"   (included in some Orthodox/Slavonic traditions)
    PrayerOfManasseh, // "pmn"
    Psalm151,         // "ps151"
    ThirdMaccabees,   // "3mc"
    FourthMaccabees,  // "4mc"   (often appendix)
}

impl BibleBook {
    /// Returns the compact abbreviation for this Bible book (e.g., "gn", "jdt", "ps151").
    pub const fn as_str(&self) -> &'static str {
        match self {
            // --- Protestant (66) ---
            BibleBook::Genesis => "gn",
            BibleBook::Exodus => "ex",
            BibleBook::Leviticus => "lv",
            BibleBook::Numbers => "nm",
            BibleBook::Deuteronomy => "dt",
            BibleBook::Joshua => "js",
            BibleBook::Judges => "jud",
            BibleBook::Ruth => "rt",
            BibleBook::FirstSamuel => "1sm",
            BibleBook::SecondSamuel => "2sm",
            BibleBook::FirstKings => "1kgs",
            BibleBook::SecondKings => "2kgs",
            BibleBook::FirstChronicles => "1ch",
            BibleBook::SecondChronicles => "2ch",
            BibleBook::Ezra => "ezr",
            BibleBook::Nehemiah => "ne",
            BibleBook::Esther => "et",
            BibleBook::Job => "job",
            BibleBook::Psalms => "ps",
            BibleBook::Proverbs => "prv",
            BibleBook::Ecclesiastes => "ec",
            BibleBook::SongOfSolomon => "so",
            BibleBook::Isaiah => "is",
            BibleBook::Jeremiah => "jr",
            BibleBook::Lamentations => "lm",
            BibleBook::Ezekiel => "ez",
            BibleBook::Daniel => "dn",
            BibleBook::Hosea => "ho",
            BibleBook::Joel => "jl",
            BibleBook::Amos => "am",
            BibleBook::Obadiah => "ob",
            BibleBook::Jonah => "jn",
            BibleBook::Micah => "mi",
            BibleBook::Nahum => "na",
            BibleBook::Habakkuk => "hk",
            BibleBook::Zephaniah => "zp",
            BibleBook::Haggai => "hg",
            BibleBook::Zechariah => "zc",
            BibleBook::Malachi => "ml",
            BibleBook::Matthew => "mt",
            BibleBook::Mark => "mk",
            BibleBook::Luke => "lk",
            BibleBook::John => "jo",
            BibleBook::Acts => "act",
            BibleBook::Romans => "rm",
            BibleBook::FirstCorinthians => "1co",
            BibleBook::SecondCorinthians => "2co",
            BibleBook::Galatians => "gl",
            BibleBook::Ephesians => "eph",
            BibleBook::Philippians => "ph",
            BibleBook::Colossians => "cl",
            BibleBook::FirstThessalonians => "1ts",
            BibleBook::SecondThessalonians => "2ts",
            BibleBook::FirstTimothy => "1tm",
            BibleBook::SecondTimothy => "2tm",
            BibleBook::Titus => "tt",
            BibleBook::Philemon => "phm",
            BibleBook::Hebrews => "hb",
            BibleBook::James => "jm",
            BibleBook::FirstPeter => "1pe",
            BibleBook::SecondPeter => "2pe",
            BibleBook::FirstJohn => "1jo",
            BibleBook::SecondJohn => "2jo",
            BibleBook::ThirdJohn => "3jo",
            BibleBook::Jude => "jd",
            BibleBook::Revelation => "re",

            // --- Catholic Deuterocanon ---
            BibleBook::Tobit => "tb",
            BibleBook::Judith => "jdt",
            BibleBook::Wisdom => "ws",
            BibleBook::Sirach => "sir",
            BibleBook::Baruch => "bar",
            BibleBook::FirstMaccabees => "1mc",
            BibleBook::SecondMaccabees => "2mc",
            BibleBook::EstherAdditions => "etg",
            BibleBook::DanielSongOfThree => "dn3",
            BibleBook::DanielSusanna => "dns",
            BibleBook::DanielBelAndTheDragon => "dnb",

            // --- Eastern Orthodox Additions ---
            BibleBook::FirstEsdras => "1es",
            BibleBook::SecondEsdras => "2es",
            BibleBook::PrayerOfManasseh => "pmn",
            BibleBook::Psalm151 => "ps151",
            BibleBook::ThirdMaccabees => "3mc",
            BibleBook::FourthMaccabees => "4mc",
        }
    }
}

impl core::fmt::Display for BibleBook {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned when parsing an unknown/unsupported abbreviation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseBibleBookError;

impl FromStr for BibleBook {
    type Err = ParseBibleBookError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_ascii_lowercase(); // own the lowercase string
        let s = s.as_str();

        let book = match s {
            // --- Protestant (66) ---
            "gn" => BibleBook::Genesis,
            "ex" => BibleBook::Exodus,
            "lv" => BibleBook::Leviticus,
            "nm" => BibleBook::Numbers,
            "dt" => BibleBook::Deuteronomy,
            "js" => BibleBook::Joshua,
            "jud" => BibleBook::Judges,
            "rt" => BibleBook::Ruth,
            "1sm" => BibleBook::FirstSamuel,
            "2sm" => BibleBook::SecondSamuel,
            "1kgs" => BibleBook::FirstKings,
            "2kgs" => BibleBook::SecondKings,
            "1ch" => BibleBook::FirstChronicles,
            "2ch" => BibleBook::SecondChronicles,
            "ezr" => BibleBook::Ezra,
            "ne" => BibleBook::Nehemiah,
            "et" => BibleBook::Esther,
            "job" => BibleBook::Job,
            "ps" => BibleBook::Psalms,
            "prv" => BibleBook::Proverbs,
            "ec" => BibleBook::Ecclesiastes,
            "so" => BibleBook::SongOfSolomon,
            "is" => BibleBook::Isaiah,
            "jr" => BibleBook::Jeremiah,
            "lm" => BibleBook::Lamentations,
            "ez" => BibleBook::Ezekiel,
            "dn" => BibleBook::Daniel,
            "ho" => BibleBook::Hosea,
            "jl" => BibleBook::Joel,
            "am" => BibleBook::Amos,
            "ob" => BibleBook::Obadiah,
            "jn" => BibleBook::Jonah,
            "mi" => BibleBook::Micah,
            "na" => BibleBook::Nahum,
            "hk" => BibleBook::Habakkuk,
            "zp" => BibleBook::Zephaniah,
            "hg" => BibleBook::Haggai,
            "zc" => BibleBook::Zechariah,
            "ml" => BibleBook::Malachi,
            "mt" => BibleBook::Matthew,
            "mk" => BibleBook::Mark,
            "lk" => BibleBook::Luke,
            "jo" => BibleBook::John,
            "act" => BibleBook::Acts,
            "rm" => BibleBook::Romans,
            "1co" => BibleBook::FirstCorinthians,
            "2co" => BibleBook::SecondCorinthians,
            "gl" => BibleBook::Galatians,
            "eph" => BibleBook::Ephesians,
            "ph" => BibleBook::Philippians,
            "cl" => BibleBook::Colossians,
            "1ts" => BibleBook::FirstThessalonians,
            "2ts" => BibleBook::SecondThessalonians,
            "1tm" => BibleBook::FirstTimothy,
            "2tm" => BibleBook::SecondTimothy,
            "tt" => BibleBook::Titus,
            "phm" => BibleBook::Philemon,
            "hb" => BibleBook::Hebrews,
            "jm" => BibleBook::James,
            "1pe" => BibleBook::FirstPeter,
            "2pe" => BibleBook::SecondPeter,
            "1jo" => BibleBook::FirstJohn,
            "2jo" => BibleBook::SecondJohn,
            "3jo" => BibleBook::ThirdJohn,
            "jd" => BibleBook::Jude,
            "re" => BibleBook::Revelation,

            // --- Catholic Deuterocanon ---
            "tb" => BibleBook::Tobit,
            "jdt" => BibleBook::Judith,
            "ws" => BibleBook::Wisdom,
            "sir" => BibleBook::Sirach,
            "bar" => BibleBook::Baruch,
            "1mc" => BibleBook::FirstMaccabees,
            "2mc" => BibleBook::SecondMaccabees,
            "etg" => BibleBook::EstherAdditions,
            "dn3" => BibleBook::DanielSongOfThree,
            "dns" => BibleBook::DanielSusanna,
            "dnb" => BibleBook::DanielBelAndTheDragon,

            // --- Eastern Orthodox Additions ---
            "1es" => BibleBook::FirstEsdras,
            "2es" => BibleBook::SecondEsdras,
            "pmn" => BibleBook::PrayerOfManasseh,
            "ps151" => BibleBook::Psalm151,
            "3mc" => BibleBook::ThirdMaccabees,
            "4mc" => BibleBook::FourthMaccabees,

            _ => return Err(ParseBibleBookError),
        };

        Ok(book)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::str::FromStr;

    #[test]
    fn roundtrip_examples() {
        let codes = [
            ("gn", BibleBook::Genesis),
            ("JO", BibleBook::John), // case-insensitive
            ("1mc", BibleBook::FirstMaccabees),
            ("ps151", BibleBook::Psalm151),
            ("dns", BibleBook::DanielSusanna),
        ];

        for (code, book) in codes {
            let parsed = BibleBook::from_str(code).unwrap();
            assert_eq!(parsed, book);
            assert_eq!(book.as_str(), book.to_string());
        }
    }

    #[test]
    fn reject_unknown() {
        assert!(BibleBook::from_str("xyz").is_err());
    }
}

#[test]
fn test_bible_book_as_str() {
    assert_eq!(BibleBook::Genesis.as_str(), "gn");
    assert_eq!(BibleBook::Exodus.as_str(), "ex");
    assert_eq!(BibleBook::Psalms.as_str(), "ps");
    assert_eq!(BibleBook::Matthew.as_str(), "mt");
    assert_eq!(BibleBook::Revelation.as_str(), "re");
}

#[test]
fn test_bible_book_display() {
    assert_eq!(format!("{}", BibleBook::Genesis), "gn");
    assert_eq!(format!("{}", BibleBook::Psalms), "ps");
    assert_eq!(format!("{}", BibleBook::Revelation), "re");
}

#[test]
fn test_bible_book_from_str() {
    assert_eq!(BibleBook::from_str("gn"), Ok(BibleBook::Genesis));
    assert_eq!(BibleBook::from_str("ps"), Ok(BibleBook::Psalms));
    assert_eq!(BibleBook::from_str("mt"), Ok(BibleBook::Matthew));
    assert_eq!(BibleBook::from_str("re"), Ok(BibleBook::Revelation));

    // Test invalid strings
    assert_eq!(BibleBook::from_str("invalid"), Err(ParseBibleBookError));
    assert_eq!(BibleBook::from_str(""), Err(ParseBibleBookError));
}

#[test]
fn test_bible_book_debug() {
    // Test that Debug trait works
    let book = BibleBook::Genesis;
    let debug_str = format!("{:?}", book);
    assert!(debug_str.contains("Genesis"));
}

#[test]
fn test_bible_book_clone_copy() {
    // Test Clone and Copy traits
    let book1 = BibleBook::Genesis;
    let book2 = book1; // Copy
    #[allow(clippy::clone_on_copy)]
    let book3 = book1.clone(); // Clone

    assert_eq!(book1, book2);
    assert_eq!(book1, book3);
    assert_eq!(book2, book3);
}

#[test]
fn test_specific_book_abbreviations() {
    // Test some specific abbreviations to ensure they're correct
    assert_eq!(BibleBook::FirstSamuel.as_str(), "1sm");
    assert_eq!(BibleBook::SecondSamuel.as_str(), "2sm");
    assert_eq!(BibleBook::FirstKings.as_str(), "1kgs");
    assert_eq!(BibleBook::SecondKings.as_str(), "2kgs");
    assert_eq!(BibleBook::FirstCorinthians.as_str(), "1co");
    assert_eq!(BibleBook::SecondCorinthians.as_str(), "2co");
    assert_eq!(BibleBook::FirstJohn.as_str(), "1jo");
    assert_eq!(BibleBook::SecondJohn.as_str(), "2jo");
    assert_eq!(BibleBook::ThirdJohn.as_str(), "3jo");
}
