use crate::rng::Rng;

/// Letter pairs in the spirit of Elite's name generator ("Lave", "Diso", "Riedquat").
/// Empty entries make names shorter.
const PAIRS: [&str; 32] = [
    "", "le", "xe", "ge", "za", "ce", "bi", "so", "us", "es", "ar", "ma", "in", "di", "re", "a", "er", "at",
    "en", "be", "ra", "la", "ve", "ti", "ed", "or", "qu", "an", "te", "is", "ri", "on",
];

pub fn star_name(seed: u64) -> String {
    let mut rng = Rng::new(seed ^ 0x6e61_6d65);
    loop {
        let parts = rng.int(2, 4);
        let name: String = (0..parts).map(|_| *rng.pick(&PAIRS)).collect();
        if name.len() >= 3 {
            let mut chars = name.chars();
            let first = chars.next().unwrap().to_ascii_uppercase();
            return std::iter::once(first).chain(chars).collect();
        }
    }
}

pub fn roman(n: usize) -> &'static str {
    ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"].get(n).copied().unwrap_or("X+")
}
