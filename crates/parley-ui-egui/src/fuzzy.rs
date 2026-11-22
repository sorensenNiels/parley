//! Finding as one types, forgiving a typo: Parley SSH's command palette (R-SSH-48) and the start screens' search
//! (R-SSH-65, R-UI-42, R-SQL-105; D64).

/// How well `query` fits `text`, higher better; `None` when it does not. Whole words first, then the letters in
/// order (`nwtb` finds "New tab"), then a word with one typo (two in a long query).
pub fn score(query: &str, text: &str) -> Option<u32> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    let text = text.to_lowercase();
    if let Some(at) = text.find(&query) {
        let at = u32::try_from(at).unwrap_or(u32::MAX).min(500);
        let starts_word = at == 0 || !text[..at as usize].ends_with(char::is_alphanumeric);
        return Some(3000 - at + if starts_word { 500 } else { 0 });
    }
    if let Some(gaps) = in_order(&query, &text) {
        return Some(2000_u32.saturating_sub(gaps));
    }
    let q: Vec<char> = query.chars().collect();
    if q.len() < 4 {
        return None;
    }
    let allowed = if q.len() >= 8 { 2 } else { 1 };
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .filter_map(|word| {
            let w: Vec<char> = word.chars().collect();
            // The query against the word's start of about the same length.
            let near = w.len().min(q.len() + 1);
            (q.len().saturating_sub(1)..=near)
                .filter(|n| *n <= w.len())
                .map(|n| distance(&q, &w[..n]))
                .min()
        })
        .min()
        .filter(|d| *d <= allowed)
        .map(|d| 1000 - u32::try_from(d).unwrap_or(1) * 100)
}

/// The letters of `query` in order in `text`: how many letters lie between them.
fn in_order(query: &str, text: &str) -> Option<u32> {
    let mut gaps = 0;
    let mut rest = text.chars();
    for q in query.chars().filter(|c| !c.is_whitespace()) {
        let mut skipped = 0;
        loop {
            match rest.next() {
                Some(c) if c == q => break,
                Some(_) => skipped += 1,
                None => return None,
            }
        }
        gaps += skipped;
    }
    Some(gaps)
}

/// Optimal string alignment distance: insertions, deletions, changes and two letters swapped.
fn distance(a: &[char], b: &[char]) -> usize {
    let mut rows = vec![vec![0; b.len() + 1]; a.len() + 1];
    for (i, row) in rows.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in rows[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let change = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (rows[i - 1][j] + 1)
                .min(rows[i][j - 1] + 1)
                .min(rows[i - 1][j - 1] + change);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(rows[i - 2][j - 2] + 1);
            }
            rows[i][j] = best;
        }
    }
    rows[a.len()][b.len()]
}

/// How well `search` fits a row named `name` with the other words `other`: the name as the palette reads a label,
/// forgiving a typo (R-SSH-48), the others only when they hold it. The start screens' search (R-SSH-65, R-UI-42,
/// R-SQL-105).
pub fn fit(search: &str, name: &str, other: &[&str]) -> Option<u32> {
    let lower = search.trim().to_lowercase();
    let holds = other
        .iter()
        .any(|o| o.to_lowercase().contains(&lower))
        .then_some(1500);
    score(search, name).map(|s| s + 100).max(holds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_letters_in_order_and_one_typo_are_found_best_first_r_ssh_48() {
        assert!(score("new tab", "New tab") > score("tab", "New tab"));
        assert!(score("tab", "New tab").is_some());
        assert!(score("nwtb", "New tab").is_some(), "letters in order");
        assert!(
            score("prdo", "Production web").is_some(),
            "two letters swapped"
        );
        assert!(
            score("prodcution", "Production web").is_some(),
            "a long query, two typos"
        );
        assert!(score("xyz", "New tab").is_none());
        assert!(score("ab", "Bab").is_some());
        assert!(
            score("set", "Session").is_none(),
            "no typo in three letters"
        );
        assert!(score("æøå", "Server æøå").is_some());
        assert_eq!(score("  ", "anything"), Some(0));
    }

    #[test]
    fn a_name_forgives_a_typo_and_the_other_words_count_when_they_hold_the_search_r_ui_42() {
        assert!(fit("prdo", "Production", &[]).is_some());
        assert!(fit("10.0.0", "Prod", &["10.0.0.5:992"]).is_some());
        assert!(fit("10.0.1", "Prod", &["10.0.0.5:992"]).is_none());
        assert!(fit("prod", "Prod", &[]) > fit("prod", "Other", &["prod.lan"]));
        assert!(fit("ÆØÅ", "æøå test", &[]).is_some());
    }
}
