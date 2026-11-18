use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::Arc;
use serde::{Serialize, Serializer};
use crate::models::Endpoint;
use crate::routing::pattern::{split_path_and_query, RoutePattern};

/// A registered endpoint together with the pattern requests are matched against.
#[derive(Debug)]
pub struct Route {
    /// The path exactly as it was registered, which is also its identity in the store.
    pub path: String,
    pub pattern: RoutePattern,
    pub endpoint: Endpoint,
}

/// Every registered endpoint, indexed so a request is resolved without scanning them all.
///
/// Routes with a fixed path are found by a hash lookup. Only routes with placeholders in
/// the path are tried one by one, and only when no fixed path matched.
#[derive(Debug, Default)]
pub struct EndpointStore {
    routes: HashMap<String, Arc<Route>>,
    literal: HashMap<String, Vec<Arc<Route>>>,
    templated: Vec<Arc<Route>>,
}

impl EndpointStore {
    /// Registers an endpoint, returning the route it replaced when the path was taken.
    pub fn insert(&mut self, path: String, endpoint: Endpoint) -> Option<Arc<Route>> {
        let replaced = self.remove(&path);
        let pattern = RoutePattern::parse(&path);
        let route = Arc::new(Route { path: path.clone(), pattern, endpoint });

        let candidates = if route.pattern.has_literal_path() {
            let (literal_path, _) = split_path_and_query(&path);
            self.literal.entry(literal_path.to_string()).or_default()
        } else {
            &mut self.templated
        };
        candidates.push(Arc::clone(&route));
        // The most specific route wins: fewer placeholders, then more required query values.
        candidates.sort_by_key(|candidate| {
            (candidate.pattern.path_param_count(), Reverse(candidate.pattern.required_query_count()))
        });

        self.routes.insert(path, route);

        replaced
    }

    pub fn remove(&mut self, path: &str) -> Option<Arc<Route>> {
        let removed = self.routes.remove(path)?;

        if removed.pattern.has_literal_path() {
            let (literal_path, _) = split_path_and_query(path);

            if let Some(candidates) = self.literal.get_mut(literal_path) {
                candidates.retain(|candidate| !Arc::ptr_eq(candidate, &removed));

                if candidates.is_empty() {
                    self.literal.remove(literal_path);
                }
            }
        } else {
            self.templated.retain(|candidate| !Arc::ptr_eq(candidate, &removed));
        }

        Some(removed)
    }

    /// Resolves a request to the most specific route that accepts its path and query.
    pub fn find(&self, path: &str, query: &str) -> Option<Arc<Route>> {
        let literal_match = self.literal
            .get(path)
            .and_then(|candidates| candidates
                .iter()
                .find(|candidate| candidate.pattern.matches_query(query)));

        literal_match
            .or_else(|| self.templated
                .iter()
                .find(|candidate| candidate.pattern.matches_path(path) && candidate.pattern.matches_query(query)))
            .cloned()
    }

    /// Looks a route up by the path it was registered with.
    pub fn get(&self, path: &str) -> Option<&Arc<Route>> {
        self.routes.get(path)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<Route>> {
        self.routes.values()
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

/// Serializes as a map from registered path to endpoint, the shape the dashboard lists.
impl Serialize for EndpointStore {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.routes
            .iter()
            .map(|(path, route)| (path, &route.endpoint)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::EndpointKind;

    fn endpoint(file: &str) -> Endpoint {
        Endpoint {
            kind: EndpointKind::Rest,
            method: vec!["GET".to_string()],
            file: file.to_string(),
            status_code: None,
            authentication: None,
            delay: None,
            rate_limit: None,
            with_dynamic_vars: None,
        }
    }

    fn store_with(paths: &[&str]) -> EndpointStore {
        let mut store = EndpointStore::default();

        for path in paths {
            store.insert(path.to_string(), endpoint(path));
        }

        store
    }

    fn found(store: &EndpointStore, path: &str, query: &str) -> Option<String> {
        store.find(path, query).map(|route| route.path.clone())
    }

    #[test]
    fn finds_a_literal_path() {
        let store = store_with(&["/api/hello"]);

        assert_eq!(found(&store, "/api/hello", ""), Some("/api/hello".to_string()));
        assert_eq!(found(&store, "/api/hello", "page=2"), Some("/api/hello".to_string()));
        assert_eq!(found(&store, "/api/other", ""), None);
    }

    #[test]
    fn finds_a_templated_path() {
        let store = store_with(&["/api/user/{id}"]);

        assert_eq!(found(&store, "/api/user/42", ""), Some("/api/user/{id}".to_string()));
        assert_eq!(found(&store, "/api/user/42/orders", ""), None);
    }

    #[test]
    fn literal_path_wins_over_a_template() {
        let store = store_with(&["/api/user/{id}", "/api/user/me"]);

        assert_eq!(found(&store, "/api/user/me", ""), Some("/api/user/me".to_string()));
        assert_eq!(found(&store, "/api/user/42", ""), Some("/api/user/{id}".to_string()));
    }

    #[test]
    fn query_values_select_between_routes_on_the_same_path() {
        let store = store_with(&["/search", "/search?kind=book", "/search?kind=book&lang=en"]);

        assert_eq!(found(&store, "/search", "kind=film"), Some("/search".to_string()));
        assert_eq!(found(&store, "/search", "kind=book"), Some("/search?kind=book".to_string()));
        assert_eq!(
            found(&store, "/search", "lang=en&kind=book"),
            Some("/search?kind=book&lang=en".to_string())
        );
    }

    #[test]
    fn registering_a_path_again_replaces_it() {
        let mut store = store_with(&["/api/hello"]);

        let replaced = store.insert("/api/hello".to_string(), endpoint("second"));

        assert_eq!(replaced.map(|route| route.endpoint.file.clone()), Some("/api/hello".to_string()));
        assert_eq!(store.len(), 1);
        assert_eq!(store.find("/api/hello", "").unwrap().endpoint.file, "second");
    }

    #[test]
    fn removed_routes_are_no_longer_found() {
        let mut store = store_with(&["/api/hello", "/api/user/{id}"]);

        store.remove("/api/hello");
        store.remove("/api/user/{id}");

        assert!(store.is_empty());
        assert_eq!(found(&store, "/api/hello", ""), None);
        assert_eq!(found(&store, "/api/user/42", ""), None);
    }
}
