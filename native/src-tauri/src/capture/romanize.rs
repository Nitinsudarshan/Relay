//! Devanagari to Latin script, for people who speak Hindi and cannot read it.
//!
//! Relay never translates: a Hindi meeting is transcribed as Hindi, because
//! turning it into English would put words in someone's mouth. But a user who
//! speaks Hindi and reads only Latin script gets a transcript they cannot use —
//! the words are right and the alphabet is wrong. `Settings › Languages &
//! Script` has offered to fix exactly this since before this module existed;
//! nothing implemented it.
//!
//! # Why this is written here rather than taken from a crate
//!
//! Devanagari romanization needs state, and the available crates
//! (`any_ascii`, `deunicode`) are per-codepoint lookup tables. A consonant
//! carries an inherent *a* which the *following* character may replace (a
//! vowel sign) or suppress (a virama). Handed क्या a stateless table emits
//! "kaya"; the word is "kya". Getting that wrong on every consonant cluster
//! produces text no more readable than the Devanagari it replaced.
//!
//! # What this is not
//!
//! Not a reversible scholarly transliteration. IAST and ISO 15919 exist for
//! that and both spend diacritics — ā, ṭ, ṣ — on distinctions this reader does
//! not need and cannot type. The target here is what a Hindi speaker actually
//! writes in Latin script: "bhej dungi", not "bheja dūṅgī". Retroflex and
//! dental consonants both become `t`/`d`, long and short *i*/*u* both become
//! `i`/`u`, because that is the convention the reader already knows.
//!
//! Being lossy is safe *because* it is a projection. The Devanagari is what
//! Relay stores; this is only what it shows. Turn the setting off and the
//! original is still there, byte for byte.

use std::borrow::Cow;

/// Which alphabet a surface should render text in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputScript {
    /// Romanized. The default, and what a Latin-only reader needs — a
    /// transcript in an alphabet the reader cannot read is the failure this
    /// module exists to prevent, so it is the safer of the two defaults.
    #[default]
    Latin,
    /// As spoken and as stored. For a reader who can read the original.
    Native,
}

impl OutputScript {
    /// Parses the persisted `output_script` setting, defaulting rather than
    /// failing — an unreadable transcript is a worse outcome than an
    /// unrecognised setting value.
    pub fn from_setting(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "native" => Self::Native,
            _ => Self::Latin,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Latin => "latin",
            Self::Native => "native",
        }
    }
}

/// Renders `text` in `script`.
///
/// The projection entry point, and the only function callers should need.
/// Borrows when there is nothing to do, which is every English transcript and
/// every transcript at all when the setting is `Native`.
pub fn project(text: &str, script: OutputScript) -> Cow<'_, str> {
    match script {
        OutputScript::Native => Cow::Borrowed(text),
        OutputScript::Latin if !contains_devanagari(text) => Cow::Borrowed(text),
        OutputScript::Latin => Cow::Owned(to_latin(text)),
    }
}

/// Applies the script projection to every string in a serialized payload.
///
/// Used at the command boundary, where a whole meeting — transcript segments,
/// summary markdown, action items, topics, speaker labels — crosses to the UI
/// in one value. Walking the JSON rather than naming each field means a field
/// added later is projected too, instead of being the one place Devanagari
/// still leaks through.
///
/// Safe precisely because [`to_latin`] only touches Devanagari: segment ids,
/// file paths, timestamps and enum tags are ASCII and pass through untouched,
/// so this cannot corrupt an identifier. Object *keys* are never projected.
pub fn project_json(value: &mut serde_json::Value, script: OutputScript) {
    if script == OutputScript::Native {
        return;
    }
    match value {
        serde_json::Value::String(s) => {
            if contains_devanagari(s) {
                *s = to_latin(s);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                project_json(item, script);
            }
        }
        serde_json::Value::Object(map) => {
            for (_key, v) in map.iter_mut() {
                project_json(v, script);
            }
        }
        _ => {}
    }
}

/// Whether `text` holds anything this module would change.
///
/// Checked before allocating, because the common case is text that is already
/// Latin and must be returned untouched.
pub fn contains_devanagari(text: &str) -> bool {
    text.chars().any(is_devanagari)
}

/// The Devanagari block, plus the extended block that carries a few Hindi
/// letters.
fn is_devanagari(c: char) -> bool {
    matches!(c, '\u{0900}'..='\u{097F}' | '\u{A8E0}'..='\u{A8FF}')
}

/// Romanizes Devanagari, passing everything else through unchanged.
///
/// Code-switched text is the normal case, not an edge case: a Hinglish
/// sentence carries English words mid-clause, and they must survive untouched.
pub fn to_latin(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() * 2);
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Zero-width joiners control ligature shaping and carry no sound.
        if matches!(c, ZWJ | ZWNJ) {
            i += 1;
            continue;
        }

        if let Some(base) = consonant(c) {
            i = write_syllable(&mut out, &chars, i, base);
            continue;
        }

        if let Some(v) = independent_vowel(c) {
            out.push_str(v);
            i += 1;
            i = write_trailing_signs(&mut out, &chars, i);
            continue;
        }

        if let Some(d) = digit(c) {
            out.push(d);
            i += 1;
            continue;
        }

        match c {
            // Danda and double danda are full stops.
            '\u{0964}' | '\u{0965}' => out.push('.'),
            // Avagraha marks an elided vowel; an apostrophe is the closest
            // thing a Latin reader will recognise.
            '\u{093D}' => out.push('\''),
            // Vedic accents and the nukta on its own carry no sound here.
            '\u{0951}'..='\u{0954}' | NUKTA => {}
            // A stray vowel sign or virama with no consonant before it is
            // malformed input. Dropping it is better than emitting a glyph the
            // reader cannot place.
            _ if matra(c).is_some() || c == VIRAMA => {}
            _ if is_devanagari(c) => {}
            // Everything else — Latin, digits, punctuation, emoji — is not
            // ours to touch.
            _ => out.push(c),
        }
        i += 1;
    }

    out
}

/// Writes one consonant and whatever vowel belongs to it, returning the index
/// to continue from.
fn write_syllable(out: &mut String, chars: &[char], start: usize, base: &str) -> usize {
    let mut i = start + 1;
    let mut base = base;

    // A nukta on the next position changes which consonant this is.
    if chars.get(i) == Some(&NUKTA) {
        if let Some(n) = nukta_form(chars[start]) {
            base = n;
        }
        i += 1;
    }
    out.push_str(base);

    match chars.get(i) {
        // A virama suppresses the inherent vowel: this consonant joins the
        // next one. This is the rule a lookup table cannot express, and the
        // reason क्या is "kya".
        Some(&VIRAMA) => return i + 1,
        Some(&next) if matra(next).is_some() => {
            let vowel = matra(next).unwrap_or_default();
            // Word-final आ reads better short: क्या is "kya", not "kyaa".
            // Mid-word it stays long, so राम is "raam" rather than "ram".
            let ends_word = !chars
                .get(i + 1)
                .is_some_and(|&c| is_devanagari(c) && c != VIRAMA);
            if ends_word && vowel == "aa" {
                out.push('a');
            } else {
                out.push_str(vowel);
            }
            i += 1;
        }
        _ => {
            // The inherent vowel, dropped at the end of a word — Hindi does
            // not pronounce it there, and "Ram" is the name the reader knows,
            // not "Rama".
            if !at_word_end(chars, i) {
                out.push('a');
            }
        }
    }

    write_trailing_signs(out, chars, i)
}

/// True when no Devanagari letter follows, so a word boundary falls here.
///
/// A combining sign (anusvara, visarga) does not end a word — it belongs to
/// the syllable just written, and is emitted after the vowel.
fn at_word_end(chars: &[char], i: usize) -> bool {
    let mut j = i;
    while let Some(&c) = chars.get(j) {
        if is_combining_sign(c) || matches!(c, ZWJ | ZWNJ) {
            j += 1;
            continue;
        }
        return !is_devanagari_letter(c);
    }
    true
}

/// Emits anusvara, chandrabindu and visarga, which trail the vowel they nasalize
/// or aspirate.
fn write_trailing_signs(out: &mut String, chars: &[char], start: usize) -> usize {
    let mut i = start;
    while let Some(&c) = chars.get(i) {
        match c {
            // Both nasalize. Hindi speakers write both as `n`.
            ANUSVARA | CHANDRABINDU => out.push('n'),
            VISARGA => out.push('h'),
            _ => break,
        }
        i += 1;
    }
    i
}

fn is_combining_sign(c: char) -> bool {
    matches!(c, ANUSVARA | CHANDRABINDU | VISARGA | NUKTA | '\u{0951}'..='\u{0954}')
}

/// A letter that can stand as part of a word, as opposed to a mark on one.
fn is_devanagari_letter(c: char) -> bool {
    consonant(c).is_some() || independent_vowel(c).is_some() || matra(c).is_some() || c == VIRAMA
}

const VIRAMA: char = '\u{094D}';
const NUKTA: char = '\u{093C}';
const ANUSVARA: char = '\u{0902}';
const CHANDRABINDU: char = '\u{0901}';
const VISARGA: char = '\u{0903}';
const ZWNJ: char = '\u{200C}';
const ZWJ: char = '\u{200D}';

/// Consonants, without their inherent vowel.
///
/// Retroflex and dental collapse (ट and त are both `t`), as do the two
/// sibilants (श and ष are both `sh`). That is not sloppiness: it is the
/// convention every Hindi speaker uses when typing Latin, and the distinction
/// costs a diacritic this reader did not ask for.
fn consonant(c: char) -> Option<&'static str> {
    Some(match c {
        'क' => "k",
        'ख' => "kh",
        'ग' => "g",
        'घ' => "gh",
        'ङ' => "ng",
        'च' => "ch",
        'छ' => "chh",
        'ज' => "j",
        'झ' => "jh",
        'ञ' => "ny",
        'ट' => "t",
        'ठ' => "th",
        'ड' => "d",
        'ढ' => "dh",
        'ण' => "n",
        'त' => "t",
        'थ' => "th",
        'द' => "d",
        'ध' => "dh",
        'न' => "n",
        'ऩ' => "n",
        'प' => "p",
        'फ' => "ph",
        'ब' => "b",
        'भ' => "bh",
        'म' => "m",
        'य' => "y",
        'र' => "r",
        'ऱ' => "r",
        'ल' => "l",
        'ळ' => "l",
        'ऴ' => "l",
        'व' => "v",
        'श' => "sh",
        'ष' => "sh",
        'स' => "s",
        'ह' => "h",
        // Precomposed nukta letters, common in loanwords.
        '\u{0958}' => "q",
        '\u{0959}' => "kh",
        '\u{095A}' => "g",
        '\u{095B}' => "z",
        '\u{095C}' => "r",
        '\u{095D}' => "rh",
        '\u{095E}' => "f",
        '\u{095F}' => "y",
        _ => return None,
    })
}

/// The consonant a base letter becomes when a separate nukta follows it.
///
/// The same sounds as the precomposed letters above; Unicode allows either
/// spelling and Whisper emits both.
fn nukta_form(c: char) -> Option<&'static str> {
    Some(match c {
        'क' => "q",
        'ख' => "kh",
        'ग' => "g",
        'ज' => "z",
        'ड' => "r",
        'ढ' => "rh",
        'फ' => "f",
        'य' => "y",
        _ => return None,
    })
}

/// Vowels that stand on their own, at the start of a word or after another vowel.
fn independent_vowel(c: char) -> Option<&'static str> {
    Some(match c {
        'अ' => "a",
        'आ' => "aa",
        'इ' => "i",
        'ई' => "i",
        'उ' => "u",
        'ऊ' => "u",
        'ऋ' => "ri",
        'ऌ' => "li",
        'ॠ' => "ri",
        'ॡ' => "li",
        'ए' => "e",
        'ऐ' => "ai",
        'ओ' => "o",
        'औ' => "au",
        // Candra vowels, used for English loanwords: डॉक्टर, ऑफ़िस.
        'ऑ' => "o",
        'ऍ' => "e",
        'ऒ' => "o",
        'ऎ' => "e",
        _ => return None,
    })
}

/// Vowel signs, which replace a consonant's inherent vowel.
fn matra(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{093E}' => "aa",
        '\u{093F}' => "i",
        '\u{0940}' => "i",
        '\u{0941}' => "u",
        '\u{0942}' => "u",
        '\u{0943}' => "ri",
        '\u{0944}' => "ri",
        '\u{0962}' => "li",
        '\u{0963}' => "li",
        '\u{0947}' => "e",
        '\u{0948}' => "ai",
        '\u{094B}' => "o",
        '\u{094C}' => "au",
        '\u{0949}' => "o",
        '\u{0945}' => "e",
        '\u{094A}' => "o",
        '\u{0946}' => "e",
        _ => return None,
    })
}

fn digit(c: char) -> Option<char> {
    match c {
        '\u{0966}'..='\u{096F}' => {
            char::from_digit(c as u32 - 0x0966, 10)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The fixtures are from a real recording — a volunteer-interview standup —
    // so the words are ones Relay actually has to render, including the names
    // that made the English summary useful.

    #[test]
    fn a_virama_joins_two_consonants() {
        // The rule a per-codepoint table cannot express, and the reason this
        // module is not a dependency.
        assert_eq!(to_latin("क्या"), "kya");
        assert_eq!(to_latin("प्रगति"), "pragati");
        assert_eq!(to_latin("स्कूल"), "skul");
    }

    #[test]
    fn a_word_final_consonant_drops_its_inherent_vowel() {
        // Hindi does not pronounce it, and the reader knows the name as "Ram".
        assert_eq!(to_latin("राम"), "raam");
        assert_eq!(to_latin("भेज"), "bhej");
        // Mid-word the inherent vowel stays.
        assert_eq!(to_latin("समय"), "samay");
    }

    #[test]
    fn anusvara_nasalizes_the_vowel_it_follows() {
        assert_eq!(to_latin("मंसी"), "mansi");
        assert_eq!(to_latin("दूंगी"), "dungi");
    }

    #[test]
    fn the_reported_sentence_becomes_readable() {
        // A loanword spelled with its nukta, as Hindi properly writes it.
        assert_eq!(
            to_latin("मैं \u{095E}ॉर्म भर दूंगी"),
            "main form bhar dungi"
        );
        assert_eq!(to_latin("तीन इंटरव्यू हो गए"), "tin intaravyu ho gae");
    }

    #[test]
    fn a_medial_inherent_vowel_is_kept_rather_than_guessed_away() {
        // Hindi deletes some medial schwas — इंटरव्यू is said "intarvyu", not
        // "intaravyu" — and the rule that decides which needs morphology this
        // module does not have.
        //
        // The tempting shortcut is "drop the schwa before a consonant
        // cluster", which does produce "intarvyu". It also turns सप्ताह into
        // "sptaah" instead of "saptaah", so it makes some words worse to make
        // others better. Keeping every inherent vowel is consistently
        // readable, which is the property that matters: a reader recognises
        // "intaravyu" as "interview" without effort.
        assert_eq!(to_latin("इंटरव्यू"), "intaravyu");
        assert_eq!(to_latin("सप्ताह"), "saptaah");
    }

    #[test]
    fn an_unmarked_loanword_reads_as_the_hindi_letter_it_was_spelled_with() {
        // A known and accepted limitation. फ is "ph" — फल is "phal", फिर is
        // "phir" — and the "f" sound is फ़, the same letter with a nukta.
        // Writers routinely drop the nukta, so "form" arrives as फॉर्म and
        // romanizes to "phorm".
        //
        // Not worked around on purpose. The alternative is guessing that a
        // candra-o marks a loanword, which would mis-romanize every native
        // word that uses one. Out of context the spelling is ambiguous to a
        // human reader too, and inventing a rule to resolve it would make some
        // words worse to make others better.
        assert_eq!(to_latin("फॉर्म"), "phorm");
        assert_eq!(to_latin("\u{095E}ॉर्म"), "form");
        // The native words the rule protects.
        assert_eq!(to_latin("फल"), "phal");
        assert_eq!(to_latin("फिर"), "phir");
    }

    #[test]
    fn code_switched_text_keeps_its_english() {
        // The normal case, not an edge case. English words mid-clause must
        // survive byte for byte.
        assert_eq!(
            to_latin("Mansi ne तीन interview लिए"),
            "Mansi ne tin interview lie"
        );
        assert_eq!(to_latin("CGPA 8.9 से ऊपर"), "CGPA 8.9 se upar");
    }

    #[test]
    fn a_danda_is_a_full_stop() {
        assert_eq!(to_latin("ठीक है।"), "thik hai.");
        assert_eq!(to_latin("हाँ॥"), "haan.");
    }

    #[test]
    fn devanagari_digits_become_arabic_ones() {
        assert_eq!(to_latin("२०२६"), "2026");
        assert_eq!(to_latin("५ बजे"), "5 baje");
    }

    #[test]
    fn a_nukta_changes_the_consonant_either_way_it_is_spelled() {
        // Precomposed, and base-plus-nukta. Whisper emits both.
        assert_eq!(to_latin("\u{095B}रूरी"), "zaruri");
        assert_eq!(to_latin("ज\u{093C}रूरी"), "zaruri");
        assert_eq!(to_latin("फ़ॉर्म"), "form");
    }

    #[test]
    fn the_json_projection_reaches_nested_text_and_spares_identifiers() {
        let mut value = serde_json::json!({
            "segments": [
                { "id": "seg_00001", "text": "मैं भेज दूंगी", "raw_text": "मैं भेज दूंगी" },
                { "id": "seg_00002", "text": "Three interviews" }
            ],
            "summary_markdown": "## Overview\nतीन इंटरव्यू हो गए",
            "audio_path": "C:/relay/meetings/2026/chunk_0001.wav",
            "chunk_count": 10,
            "मुख्य": "keys are not projected"
        });
        project_json(&mut value, OutputScript::Latin);

        assert_eq!(value["segments"][0]["text"], "main bhej dungi");
        assert_eq!(value["segments"][0]["raw_text"], "main bhej dungi");
        // An id is ASCII, so there is nothing for the projection to change.
        assert_eq!(value["segments"][0]["id"], "seg_00001");
        assert_eq!(value["segments"][1]["text"], "Three interviews");
        assert_eq!(
            value["summary_markdown"],
            "## Overview\ntin intaravyu ho gae"
        );
        assert_eq!(
            value["audio_path"],
            "C:/relay/meetings/2026/chunk_0001.wav"
        );
        assert_eq!(value["chunk_count"], 10);
        // The key itself is untouched; only values are projected.
        assert_eq!(value["मुख्य"], "keys are not projected");
    }

    #[test]
    fn the_json_projection_is_inert_on_native() {
        let original = serde_json::json!({ "text": "मैं भेज दूंगी" });
        let mut value = original.clone();
        project_json(&mut value, OutputScript::Native);
        assert_eq!(value, original);
    }

    #[test]
    fn latin_text_is_returned_untouched_and_unallocated() {
        let english = "Three interviews, nobody joined.";
        assert!(!contains_devanagari(english));
        assert!(matches!(
            project(english, OutputScript::Latin),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn the_native_setting_never_transforms_anything() {
        let hindi = "मैं फॉर्म भर दूंगी";
        assert!(matches!(
            project(hindi, OutputScript::Native),
            Cow::Borrowed(_)
        ));
        assert_eq!(project(hindi, OutputScript::Native), hindi);
    }

    #[test]
    fn the_setting_defaults_rather_than_failing() {
        assert_eq!(OutputScript::from_setting("native"), OutputScript::Native);
        assert_eq!(OutputScript::from_setting(" NATIVE "), OutputScript::Native);
        assert_eq!(OutputScript::from_setting("latin"), OutputScript::Latin);
        // Anything unrecognised romanizes: a transcript the user cannot read is
        // the worse of the two failures.
        for garbage in ["", "  ", "devanagari", "iast"] {
            assert_eq!(OutputScript::from_setting(garbage), OutputScript::Latin);
        }
        assert_eq!(OutputScript::default(), OutputScript::Latin);
    }

    #[test]
    fn romanizing_is_idempotent_because_latin_has_nothing_to_romanize() {
        // The projection may be applied twice — once by a list view and once by
        // a detail view reading the same field. It must not compound.
        let once = to_latin("मैं फॉर्म भर दूंगी");
        assert_eq!(to_latin(&once), once);
    }

    #[test]
    fn malformed_input_does_not_panic_or_leak_glyphs() {
        // A stray vowel sign or virama with no consonant before it. Whisper
        // does emit these on a bad decode.
        for stray in ["\u{093E}", "\u{094D}", "\u{0902}", "\u{093C}"] {
            let out = to_latin(stray);
            assert!(
                !out.chars().any(is_devanagari),
                "{stray:?} left Devanagari in {out:?}"
            );
        }
        assert_eq!(to_latin(""), "");
    }

    #[test]
    fn whitespace_and_structure_survive() {
        // Transcripts are line-oriented and the projection must not reflow them.
        assert_eq!(to_latin("हाँ\nनहीं"), "haan\nnahin");
        assert_eq!(to_latin("  ठीक  "), "  thik  ");
    }
}
