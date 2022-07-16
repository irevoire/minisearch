pub fn tokenize<'a>(content: &'a str) -> impl Iterator<Item = String> + 'a {
    content.split_whitespace().map(normalize)
}

fn normalize(word: &str) -> String {
    deunicode::deunicode(&word.to_lowercase())
        .chars()
        .filter(|c| c.is_alphabetic())
        .collect()
}
