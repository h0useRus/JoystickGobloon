//! Which phrase a recognized text means. The recognizer writes down whatever it hears; this picks
//! the closest phrase of the profile by letters (so "lights of" still finds "lights on" or "lights
//! off", and a different word ending still finds its phrase) and says how sure that is: close to
//! one phrase and clearly further from the others.

/// The result of one utterance.
#[derive(Debug, Clone, PartialEq)]
pub struct Heard {
    /// Index into the phrases; `None` = nothing heard or no phrases.
    pub phrase: Option<usize>,
    /// What was heard, empty for silence.
    pub text: String,
    /// How sure the match is, 0.0..=1.0 (see [`best`]).
    pub confidence: f32,
}

/// What `text` (the recognizer's) means among `phrases`.
pub fn hear(text: &str, phrases: &[String]) -> Heard {
    let found = best(text, phrases);
    Heard {
        phrase: found.map(|f| f.0),
        text: text.to_string(),
        confidence: found.map_or(0.0, |f| f.1),
    }
}

/// The phrase `text` means: its index in `phrases` and a confidence 0.0..=1.0; `None` when
/// there is no text or no phrase.
pub fn best(text: &str, phrases: &[String]) -> Option<(usize, f32)> {
    let heard = normalize(text);
    if heard.is_empty() {
        return None;
    }
    let mut scores: Vec<(usize, f32)> = phrases
        .iter()
        .enumerate()
        .map(|(i, p)| (i, similarity(&heard, &normalize(p))))
        .collect();
    scores.sort_by(|a, b| b.1.total_cmp(&a.1));
    let &(index, top) = scores.first()?;
    let runner_up = scores.get(1).map_or(0.0, |s| s.1);
    Some((index, top * clarity(top - runner_up)))
}

/// How far ahead of the next phrase the best one must be to keep its whole score: a smaller lead
/// lowers it, down to half for a tie (two phrases heard equally well).
const CLEAR_LEAD: f32 = 0.2;

fn clarity(lead: f32) -> f32 {
    (0.5 + lead / CLEAR_LEAD * 0.5).clamp(0.5, 1.0)
}

/// Lower case letters and digits, words split by single spaces; punctuation dropped.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        if !out.is_empty() {
            out.push(' ');
        }
        out.extend(word.chars().flat_map(char::to_lowercase));
    }
    out
}

/// 1.0 for the same text, 0.0 for nothing in common: edit distance over characters, relative to
/// the longer one.
fn similarity(a: &str, b: &str) -> f32 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    1.0 - distance(&a, &b) as f32 / longest as f32
}

/// Levenshtein distance, one row at a time.
fn distance(a: &[char], b: &[char]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if ca == cb {
                diagonal
            } else {
                1 + diagonal.min(above).min(row[j])
            };
            diagonal = above;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phrases(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn normalizes_case_and_punctuation() {
        assert_eq!(normalize("  Lights, ON! "), "lights on");
        assert_eq!(normalize("Фары — включить."), "фары включить");
        assert_eq!(normalize("打开地图。"), "打开地图");
        assert_eq!(normalize("..."), "");
    }

    #[test]
    fn exact_text_is_sure() {
        let p = phrases(&["landing gear", "lights on", "lights off"]);
        let (i, c) = best("Landing gear.", &p).unwrap();
        assert_eq!(i, 0);
        assert!(c > 0.95, "{c}");
    }

    #[test]
    fn a_slip_still_finds_the_phrase() {
        let p = phrases(&["landing gear", "request landing", "eject"]);
        let (i, c) = best("landing gears", &p).unwrap();
        assert_eq!(i, 0);
        assert!(c > 0.8, "{c}");
        // Another word ending.
        let p = phrases(&["фары включить", "открыть карту"]);
        let (i, c) = best("фары включи", &p).unwrap();
        assert_eq!(i, 0);
        assert!(c > 0.8, "{c}");
    }

    #[test]
    fn halfway_between_two_phrases_is_unsure() {
        let p = phrases(&["lights on", "lights off"]);
        let (_, c) = best("lights of", &p).unwrap();
        assert!(c < 0.5, "{c}");
    }

    #[test]
    fn unrelated_speech_is_unsure() {
        let p = phrases(&["landing gear", "flaps down", "eject"]);
        let (_, c) = best("what time is it", &p).unwrap();
        assert!(c < 0.5, "{c}");
    }

    #[test]
    fn nothing_heard_or_no_phrases() {
        assert_eq!(best("", &phrases(&["eject"])), None);
        assert_eq!(best("eject", &[]), None);
    }

    #[test]
    fn characters_without_spaces() {
        let p = phrases(&["打开地图", "放下起落架"]);
        assert_eq!(best("打开地图", &p).map(|r| r.0), Some(0));
    }
}
