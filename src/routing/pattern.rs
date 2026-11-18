use std::collections::HashMap;

/// One piece of a registered path or query: fixed text, or a `{name}` placeholder.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Literal(String),
    Param(String),
}

impl Token {
    fn parse(text: &str) -> Self {
        let name = text
            .strip_prefix('{')
            .and_then(|rest| rest.strip_suffix('}'))
            .filter(|name| !name.is_empty());

        match name {
            Some(name) => Token::Param(name.to_string()),
            None => Token::Literal(text.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueryRule {
    key: String,
    value: Token,
}

/// A registered path such as `/api/user/{id}/item/{item}?name={name}&lang=en`.
///
/// Path placeholders match any single segment. A query pair with a fixed value must be
/// present in the request, in any position. A query pair with a placeholder never
/// restricts the match, it only names the variable the value is exposed as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePattern {
    segments: Vec<Token>,
    query: Vec<QueryRule>,
}

impl RoutePattern {
    pub fn parse(registered_path: &str) -> Self {
        let (path, query) = split_path_and_query(registered_path);
        let segments = path_segments(path)
            .map(Token::parse)
            .collect();
        let query = form_urlencoded::parse(query.as_bytes())
            .map(|(key, value)| QueryRule { key: key.into_owned(), value: Token::parse(&value) })
            .collect();

        Self { segments, query }
    }

    /// True when the path has no placeholder, so it can be found by an exact lookup.
    pub fn has_literal_path(&self) -> bool {
        self.path_param_count() == 0
    }

    pub fn path_param_count(&self) -> usize {
        self.segments
            .iter()
            .filter(|segment| matches!(segment, Token::Param(_)))
            .count()
    }

    pub fn required_query_count(&self) -> usize {
        self.query
            .iter()
            .filter(|rule| matches!(rule.value, Token::Literal(_)))
            .count()
    }

    pub fn matches_path(&self, path: &str) -> bool {
        let mut requested = path_segments(path);

        for segment in &self.segments {
            match (segment, requested.next()) {
                (Token::Param(_), Some(_)) => {}
                (Token::Literal(expected), Some(actual)) if expected == actual => {}
                _ => return false,
            }
        }

        requested.next().is_none()
    }

    pub fn matches_query(&self, query: &str) -> bool {
        self.query
            .iter()
            .all(|rule| match &rule.value {
                Token::Param(_) => true,
                Token::Literal(expected) => form_urlencoded::parse(query.as_bytes())
                    .any(|(key, value)| key == rule.key.as_str() && value == expected.as_str()),
            })
    }

    /// Collects the values a request carries in its path and query, keyed by variable name.
    ///
    /// The path must already match this pattern.
    pub fn capture(&self, path: &str, query: &str, variables: &mut HashMap<String, String>) {
        for (segment, value) in self.segments.iter().zip(path_segments(path)) {
            if let Token::Param(name) = segment {
                let decoded = urlencoding::decode(value)
                    .map(|decoded| decoded.into_owned())
                    .unwrap_or_else(|_| value.to_string());
                variables.insert(name.clone(), decoded);
            }
        }

        for (key, value) in form_urlencoded::parse(query.as_bytes()) {
            for rule in &self.query {
                if let Token::Param(alias) = &rule.value {
                    if rule.key == key && alias.as_str() != key {
                        variables.insert(alias.clone(), value.to_string());
                    }
                }
            }

            variables.insert(key.into_owned(), value.into_owned());
        }
    }
}

/// Splits a registered path or request URL at the first `?`.
pub fn split_path_and_query(url: &str) -> (&str, &str) {
    url.split_once('?').unwrap_or((url, ""))
}

fn path_segments(path: &str) -> impl Iterator<Item = &str> {
    path.strip_prefix('/').unwrap_or(path).split('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured(pattern: &RoutePattern, path: &str, query: &str) -> HashMap<String, String> {
        let mut variables = HashMap::new();
        pattern.capture(path, query, &mut variables);

        variables
    }

    #[test]
    fn literal_path_matches_only_itself() {
        let pattern = RoutePattern::parse("/api/hello");

        assert!(pattern.has_literal_path());
        assert!(pattern.matches_path("/api/hello"));
        assert!(!pattern.matches_path("/api/hello/world"));
        assert!(!pattern.matches_path("/api"));
        assert!(!pattern.matches_path("/api/other"));
    }

    #[test]
    fn placeholder_matches_any_single_segment() {
        let pattern = RoutePattern::parse("/api/user/{id}/item/{item}");

        assert!(!pattern.has_literal_path());
        assert!(pattern.matches_path("/api/user/123/item/laptop"));
        assert!(!pattern.matches_path("/api/user/123/item"));
        assert!(!pattern.matches_path("/api/user/123/order/laptop"));
    }

    #[test]
    fn fixed_query_values_are_required_in_any_order() {
        let pattern = RoutePattern::parse("/search?kind=book&lang=en");

        assert!(pattern.matches_query("lang=en&kind=book"));
        assert!(pattern.matches_query("kind=book&lang=en&page=2"));
        assert!(!pattern.matches_query("kind=book"));
        assert!(!pattern.matches_query("kind=film&lang=en"));
    }

    #[test]
    fn query_placeholder_does_not_restrict_the_match() {
        let pattern = RoutePattern::parse("/api/user/{id}?name={name}");

        assert_eq!(pattern.required_query_count(), 0);
        assert!(pattern.matches_query(""));
        assert!(pattern.matches_query("name=John"));
    }

    #[test]
    fn capture_reads_path_and_query_values() {
        let pattern = RoutePattern::parse("/api/user/{id}/item/{item}?name={who}");

        let variables = captured(&pattern, "/api/user/123/item/big%20laptop", "name=John+Doe&page=2");

        assert_eq!(variables["id"], "123");
        assert_eq!(variables["item"], "big laptop");
        assert_eq!(variables["name"], "John Doe");
        assert_eq!(variables["who"], "John Doe");
        assert_eq!(variables["page"], "2");
    }
}
