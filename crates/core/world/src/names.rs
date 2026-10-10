use crate::rng::Rng;

/// Our own names: syllables of an onset, a vowel and (sometimes) a coda,
/// with a few soft endings. Empty onsets and codas make names flow.
const ONSETS: [&str; 28] = [
    "", "", "", "b", "br", "c", "d", "dr", "f", "g", "h", "k", "kr", "l", "m", "n", "p", "r", "s", "sh", "st", "t", "th", "tr", "v", "w", "y", "z",
];
const VOWELS: [&str; 12] = ["a", "a", "e", "e", "i", "i", "o", "o", "u", "ai", "au", "ei"];
const CODAS: [&str; 16] = ["", "", "", "", "", "l", "m", "n", "n", "r", "r", "s", "th", "k", "nd", "rn"];
const ENDINGS: [&str; 10] = ["", "", "", "", "a", "is", "on", "um", "ia", "ar"];

pub fn star_name(seed: u64) -> String {
    let mut rng = Rng::new(seed ^ 0x6e61_6d65);
    loop {
        let syllables = rng.int(2, 3);
        let mut name = String::new();
        for k in 0..syllables {
            name.push_str(rng.pick(&ONSETS));
            name.push_str(rng.pick(&VOWELS));
            // (Codas mostly at the end, so the middle stays open.)
            if k + 1 == syllables || rng.range(0.0, 1.0) < 0.3 {
                name.push_str(rng.pick(&CODAS));
            }
        }
        // A soft ending only after a consonant.
        if !name.ends_with(|c: char| "aeiouy".contains(c)) {
            name.push_str(rng.pick(&ENDINGS));
        }
        let vowels = name.chars().filter(|c| "aeiouy".contains(*c)).count();
        if (4..=9).contains(&name.len()) && !has_triple(&name) && vowels * 3 <= name.len() * 2 {
            let mut chars = name.chars();
            let first = chars.next().unwrap().to_ascii_uppercase();
            return std::iter::once(first).chain(chars).collect();
        }
    }
}

/// Three vowels or three consonants running together (hard to say).
fn has_triple(name: &str) -> bool {
    let vowel = |c: char| "aeiouy".contains(c);
    let c: Vec<char> = name.chars().collect();
    c.windows(3).any(|w| w.iter().all(|&x| vowel(x)) || w.iter().all(|&x| !vowel(x)))
}

pub fn roman(n: usize) -> &'static str {
    ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"].get(n).copied().unwrap_or("X+")
}
