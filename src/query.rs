#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Atom {
    pub text: String,
    pub anchor_start: bool,
    pub anchor_end: bool,
    pub negated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedQuery {
    pub groups: Vec<Vec<Atom>>,
    pub case_sensitive: bool,
}

impl Atom {
    /// `hay` is already in the correct case for comparison.
    fn matches(&self, hay: &str) -> bool {
        let found = match (self.anchor_start, self.anchor_end) {
            (false, false) => hay.contains(&self.text),
            (true, false) => hay.starts_with(&self.text),
            (false, true) => hay.ends_with(&self.text),
            (true, true) => hay == self.text,
        };
        found ^ self.negated
    }
}

fn parse_atom(token: &str, case_sensitive: bool) -> Option<Atom> {
    let mut t = token;
    let mut negated = false;
    let mut anchor_start = false;
    let mut anchor_end = false;

    if let Some(rest) = t.strip_prefix('!') {
        negated = true;
        t = rest;
    }
    if let Some(rest) = t.strip_prefix('\'') {
        t = rest; // literal marker; content atoms are already literal
    }
    if let Some(rest) = t.strip_prefix('^') {
        anchor_start = true;
        t = rest;
    }
    if let Some(rest) = t.strip_suffix('$') {
        anchor_end = true;
        t = rest;
    }
    if t.is_empty() {
        return None;
    }
    let text = if case_sensitive {
        t.to_string()
    } else {
        t.to_lowercase()
    };
    Some(Atom {
        text,
        anchor_start,
        anchor_end,
        negated,
    })
}

pub fn parse_content(query: &str, case_sensitive: Option<bool>) -> ParsedQuery {
    let cs = case_sensitive.unwrap_or_else(|| query.chars().any(|c| c.is_uppercase()));
    let mut groups: Vec<Vec<Atom>> = Vec::new();
    let mut or_pending = false;

    for token in query.split_whitespace() {
        if token == "|" {
            or_pending = true;
            continue;
        }
        let Some(atom) = parse_atom(token, cs) else {
            continue;
        };
        if or_pending && !groups.is_empty() {
            if let Some(last) = groups.last_mut() {
                last.push(atom);
            }
            or_pending = false;
        } else {
            groups.push(vec![atom]);
        }
    }

    ParsedQuery {
        groups,
        case_sensitive: cs,
    }
}

pub fn line_matches(q: &ParsedQuery, line: &str) -> bool {
    if q.groups.is_empty() {
        return true;
    }
    let lowered;
    let hay: &str = if q.case_sensitive {
        line
    } else {
        lowered = line.to_lowercase();
        &lowered
    };
    q.groups
        .iter()
        .all(|group| group.iter().any(|atom| atom.matches(hay)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(query: &str, line: &str) -> bool {
        line_matches(&parse_content(query, None), line)
    }

    #[test]
    fn empty_query_matches_everything() {
        assert!(matches("", "anything"));
        assert!(matches("   ", "anything"));
    }

    #[test]
    fn single_substring_and_smart_case() {
        assert!(matches("checked_add", "let x = a.checked_add(b);"));
        assert!(!matches("checked_add", "let x = a + b;"));
        assert!(matches("checked", "CHECKED"));
        assert!(!matches("Checked", "checked"));
        assert!(matches("Checked", "Checked"));
    }

    #[test]
    fn and_requires_all_terms() {
        assert!(matches("checked add", "a.checked_add(b)"));
        assert!(!matches("checked mul", "a.checked_add(b)"));
    }

    #[test]
    fn negation_excludes() {
        assert!(matches("checked !mut", "let x = checked(y)"));
        assert!(!matches("checked !mut", "let mut x = checked(y)"));
    }

    #[test]
    fn anchors_start_and_end() {
        assert!(matches("^fn", "fn main() {"));
        assert!(!matches("^fn", "  fn main() {"));
        assert!(matches("{$", "fn main() {"));
        assert!(!matches("{$", "fn main() { }"));
        assert!(matches("^}$", "}"));
        assert!(!matches("^}$", "} "));
    }

    #[test]
    fn or_group_with_pipe() {
        let q = "checked | unchecked";
        assert!(matches(q, "a.checked_add(b)"));
        assert!(matches(q, "a.unchecked_add(b)"));
        assert!(!matches(q, "a.wrapping_add(b)"));
    }

    #[test]
    fn and_of_or_groups() {
        let q = "add | sub checked";
        assert!(matches(q, "checked_add"));
        assert!(matches(q, "checked_sub"));
        assert!(!matches(q, "checked_mul"));
        assert!(!matches(q, "wrapping_add"));
    }

    #[test]
    fn explicit_case_override_beats_smart_case() {
        let q = parse_content("Checked", Some(false));
        assert!(line_matches(&q, "checked"));
        let q = parse_content("checked", Some(true));
        assert!(!line_matches(&q, "CHECKED"));
    }
}
