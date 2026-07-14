use std::collections::VecDeque;

const MAX_CHART_POINTS: usize = 240;
const MAX_SESSION_ROWS: usize = 256;

pub fn bounded_chart(values: impl IntoIterator<Item = f64>) -> Vec<f64> {
    let mut bounded = VecDeque::with_capacity(MAX_CHART_POINTS);
    for value in values {
        if bounded.len() == MAX_CHART_POINTS {
            bounded.pop_front();
        }
        bounded.push_back(value);
    }
    bounded.into_iter().collect()
}

pub fn session_page(requested: usize) -> Vec<i64> {
    let count = requested.min(MAX_SESSION_ROWS);
    (0..count).map(|index| index as i64).collect()
}

pub fn pseudo_localize(input: &str) -> String {
    if input.is_empty() {
        return "［］".to_owned();
    }

    let character_count = input.chars().count();
    let required_content = (character_count * 135).div_ceil(100);
    let padding = required_content.saturating_sub(character_count).max(3);
    let mut output = String::with_capacity(input.len().saturating_mul(4).saturating_add(8));
    output.push('［');
    for character in input.chars() {
        output.push(pseudo_character(character));
    }
    output.push(' ');
    output.extend(std::iter::repeat_n('·', padding));
    output.push('］');
    output
}

fn pseudo_character(character: char) -> char {
    match character {
        'A' => 'Å',
        'B' => 'Ɓ',
        'C' => 'Ç',
        'D' => 'Ð',
        'E' => 'Ê',
        'F' => 'Ƒ',
        'G' => 'Ğ',
        'H' => 'Ħ',
        'I' => 'Î',
        'J' => 'Ĵ',
        'K' => 'Ķ',
        'L' => 'Ŀ',
        'M' => 'Ṁ',
        'N' => 'Ñ',
        'O' => 'Ö',
        'P' => 'Þ',
        'Q' => 'Ǫ',
        'R' => 'Ŕ',
        'S' => 'Š',
        'T' => 'Ť',
        'U' => 'Û',
        'V' => 'Ṽ',
        'W' => 'Ŵ',
        'X' => 'Ẋ',
        'Y' => 'Ý',
        'Z' => 'Ž',
        'a' => 'å',
        'b' => 'ƀ',
        'c' => 'ç',
        'd' => 'ð',
        'e' => 'ê',
        'f' => 'ƒ',
        'g' => 'ğ',
        'h' => 'ħ',
        'i' => 'î',
        'j' => 'ĵ',
        'k' => 'ķ',
        'l' => 'ŀ',
        'm' => 'ṁ',
        'n' => 'ñ',
        'o' => 'ö',
        'p' => 'þ',
        'q' => 'ǫ',
        'r' => 'ŕ',
        's' => 'š',
        't' => 'ť',
        'u' => 'û',
        'v' => 'ṽ',
        'w' => 'ŵ',
        'x' => 'ẋ',
        'y' => 'ý',
        'z' => 'ž',
        other => other,
    }
}
