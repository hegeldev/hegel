use super::*;
use crate::native::re::matcher::in_category;

const CLASSES: [ChCode; 3] = [ChCode::Digit, ChCode::Space, ChCode::Word];

fn python_class(c: char, class: ChCode) -> bool {
    let category = unicodedata::general_category_char(c).as_str();
    let (major, minor) = (&category[..1], category);
    match class {
        ChCode::Digit => minor == "Nd",
        ChCode::Space => matches!(c, '\t'..='\r' | '\x1c'..=' ' | '\u{85}') || major == "Z",
        ChCode::Word => c == '_' || major == "L" || major == "N",
        _ => unreachable!(),
    }
}

#[test]
fn classes_follow_the_general_category() {
    let samples = (0..0x3000).chain((0x3000..=0x10FFFF).step_by(7));
    for cp in samples {
        let Some(c) = char::from_u32(cp) else {
            continue;
        };
        for class in CLASSES {
            let expected = python_class(c, class);
            assert_eq!(in_class(c, class), expected, "U+{cp:04X} {class:?}");
            assert_eq!(
                in_category(c, class, true),
                expected,
                "U+{cp:04X} {class:?}"
            );
            let ranges = class_ranges(class).unwrap();
            let in_ranges = ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(&cp));
            assert_eq!(in_ranges, expected, "U+{cp:04X} {class:?}");
        }
    }
}

#[test]
fn ranges_are_sorted_with_gaps_between() {
    for class in CLASSES {
        let ranges = class_ranges(class).unwrap();
        assert!(!ranges.is_empty());
        for pair in ranges.windows(2) {
            assert!(
                pair[0].0 <= pair[0].1 && pair[0].1 + 1 < pair[1].0,
                "{pair:?}"
            );
        }
    }
}

#[test]
fn classes_reach_beyond_the_basic_multilingual_plane() {
    assert!(in_class('\u{1D7CE}', ChCode::Digit));
    assert!(in_class('\u{10400}', ChCode::Word));
    assert!(!in_class('\u{1F600}', ChCode::Word));
    assert!(!in_class('\u{10FFFF}', ChCode::Space));
    assert!(in_class('\u{3000}', ChCode::Space));
    assert!(!in_class('\u{85}', ChCode::Word) && in_class('\u{85}', ChCode::Space));
}

#[test]
fn negated_classes_have_no_table() {
    for class in [ChCode::NotDigit, ChCode::NotSpace, ChCode::NotWord] {
        assert_eq!(class_ranges(class), None);
        assert!(!in_class('a', class));
    }
}
