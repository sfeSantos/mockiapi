use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use serde::{Serialize, Serializer};
use warp::http::Method;
use crate::middlewares::rate_limit::RateLimiter;
use crate::models::{Endpoint, EndpointKind};
use crate::persistence::{self, EndpointsFile};
use crate::routing::pattern::{split_path_and_query, RoutePattern};

/// A registered endpoint together with the pattern requests are matched against.
#[derive(Debug)]
pub struct Route {
    /// Identity of the registration. A path can hold several, one per set of methods.
    pub id: u64,
    /// The path exactly as it was registered.
    pub path: String,
    pub pattern: RoutePattern,
    pub endpoint: Endpoint,
    /// Present when the endpoint was registered with a rate limit.
    pub limiter: Option<RateLimiter>,
}

impl Route {
    fn shares_a_method_with(&self, methods: &[String]) -> bool {
        self.endpoint
            .method
            .iter()
            .any(|allowed| methods.iter().any(|method| method.eq_ignore_ascii_case(allowed)))
    }
}

/// Why no route answers a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Miss {
    /// Nothing is registered for the path and query.
    NoRoute,
    /// The path is registered, but not for the method of the request.
    MethodNotAllowed,
}

/// What registering an endpoint did to the store.
#[derive(Debug)]
pub struct Inserted {
    pub id: u64,
    /// The registrations the new one took the place of.
    pub replaced: Vec<Arc<Route>>,
}

/// Every registered endpoint, indexed so a request is resolved without scanning them all.
///
/// Routes with a fixed path are found by a hash lookup, and the method is then resolved
/// among the few registrations of that path. Only routes with placeholders in the path
/// are tried one by one, and only when no fixed path answered.
#[derive(Debug, Default)]
pub struct EndpointStore {
    /// Ordered by id, which is the order of registration.
    routes: BTreeMap<u64, Arc<Route>>,
    literal: HashMap<String, Vec<Arc<Route>>>,
    templated: Vec<Arc<Route>>,
    last_id: u64,
}

impl EndpointStore {
    /// Registers an endpoint.
    ///
    /// It takes the place of the registration named by `target`, and of every registration
    /// of the same path that shares a method with it. Registrations of that path for other
    /// methods stay. The new one keeps the id of `target`, or of the registration it
    /// replaced when there was exactly one.
    ///
    /// `target` does not have to exist: an endpoint restored from disk is given back the id
    /// it had, and no later registration is handed that id again.
    pub fn insert(&mut self, path: String, endpoint: Endpoint, target: Option<u64>) -> Inserted {
        let mut replaced_ids: Vec<u64> = self.overlapping(&path, &endpoint.method)
            .map(|route| route.id)
            .collect();

        if let Some(target) = target.filter(|target| !replaced_ids.contains(target)) {
            replaced_ids.push(target);
        }

        let replaced: Vec<Arc<Route>> = replaced_ids
            .iter()
            .filter_map(|id| self.remove(*id))
            .collect();
        let id = match (target, replaced.as_slice()) {
            (Some(target), _) => {
                self.last_id = self.last_id.max(target);

                target
            }
            (None, [only]) => only.id,
            (None, _) => self.next_id(),
        };
        let pattern = RoutePattern::parse(&path);
        let limiter = endpoint.rate_limit.clone().map(RateLimiter::new);
        let route = Arc::new(Route { id, path, pattern, endpoint, limiter });

        let candidates = if route.pattern.has_literal_path() {
            let (literal_path, _) = split_path_and_query(&route.path);
            self.literal.entry(literal_path.to_string()).or_default()
        } else {
            &mut self.templated
        };
        candidates.push(Arc::clone(&route));
        // The most specific route wins: fewer placeholders, then more required query values.
        candidates.sort_by_key(|candidate| {
            (candidate.pattern.path_param_count(), Reverse(candidate.pattern.required_query_count()))
        });

        self.routes.insert(id, route);

        Inserted { id, replaced }
    }

    pub fn remove(&mut self, id: u64) -> Option<Arc<Route>> {
        let removed = self.routes.remove(&id)?;

        if removed.pattern.has_literal_path() {
            let (literal_path, _) = split_path_and_query(&removed.path);

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

    /// Removes every registration of a path, whatever its methods.
    pub fn remove_path(&mut self, path: &str) -> Vec<Arc<Route>> {
        let ids: Vec<u64> = self.registered_on(path)
            .map(|route| route.id)
            .collect();

        ids.into_iter()
            .filter_map(|id| self.remove(id))
            .collect()
    }

    /// Resolves a request to the most specific route that accepts its path, query and method.
    pub fn find(&self, path: &str, query: &str, method: &Method) -> Result<Arc<Route>, Miss> {
        let mut method_refused = false;
        let literal = self.literal
            .get(path)
            .into_iter()
            .flatten()
            .filter(|candidate| candidate.pattern.matches_query(query));

        if let Some(route) = first_allowing(literal, method, &mut method_refused) {
            return Ok(Arc::clone(route));
        }

        let templated = self.templated
            .iter()
            .filter(|candidate| candidate.pattern.matches_path(path) && candidate.pattern.matches_query(query));

        if let Some(route) = first_allowing(templated, method, &mut method_refused) {
            return Ok(Arc::clone(route));
        }

        Err(if method_refused { Miss::MethodNotAllowed } else { Miss::NoRoute })
    }

    pub fn get(&self, id: u64) -> Option<&Arc<Route>> {
        self.routes.get(&id)
    }

    /// The registrations of a path, looked up by the path they were registered with.
    pub fn registered_on<'store>(&'store self, path: &'store str) -> impl Iterator<Item = &'store Arc<Route>> {
        let (literal_path, _) = split_path_and_query(path);

        self.literal
            .get(literal_path)
            .unwrap_or(&self.templated)
            .iter()
            .filter(move |route| route.path == path)
    }

    /// The registrations of a path that answer at least one of the given methods.
    pub fn overlapping<'store>(
        &'store self,
        path: &'store str,
        methods: &'store [String],
    ) -> impl Iterator<Item = &'store Arc<Route>> {
        self.registered_on(path)
            .filter(move |route| route.shares_a_method_with(methods))
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

    fn next_id(&mut self) -> u64 {
        self.last_id += 1;

        self.last_id
    }
}

/// Picks the first candidate that allows the method, noting when one was passed over for it.
fn first_allowing<'store>(
    candidates: impl Iterator<Item = &'store Arc<Route>>,
    method: &Method,
    method_refused: &mut bool,
) -> Option<&'store Arc<Route>> {
    for candidate in candidates {
        if candidate.endpoint.allows(method) {
            return Some(candidate);
        }

        *method_refused = true;
    }

    None
}

/// The endpoint store shared by every request.
///
/// Requests only read it, so they share a read lock and never wait on each other. The
/// lock is taken and released inside each method: it cannot be held across an await,
/// which is why a blocking `RwLock` is enough.
#[derive(Debug, Clone, Default)]
pub struct Endpoints {
    store: Arc<RwLock<EndpointStore>>,
    /// Where the endpoints are saved. Without it they live in memory only.
    saved: Option<Arc<EndpointsFile>>,
}

impl Endpoints {
    /// An empty store whose endpoints are written to `file` by [`Endpoints::save`].
    ///
    /// What the file already holds is not read here: see [`persistence::restore`].
    pub fn saved_in(file: impl Into<PathBuf>) -> Self {
        Self { store: Arc::default(), saved: Some(Arc::new(EndpointsFile::new(file))) }
    }

    pub fn saved_file(&self) -> Option<&EndpointsFile> {
        self.saved.as_deref()
    }

    /// Writes the endpoints to their file, after a change to what is registered.
    ///
    /// A store that lives in memory only does nothing. A write that fails is logged and the
    /// change stays valid in memory: the endpoint was registered, it is just not saved yet.
    pub async fn save(&self) {
        let Some(file) = &self.saved else {
            return;
        };

        if let Err(error) = file.write(|| persistence::snapshot(self.read().iter())).await {
            log::error!("the endpoints could not be saved to {}: {error}", file.path().display());
        }
    }

    /// Resolves a request to the most specific route that accepts its path, query and method.
    pub fn find(&self, path: &str, query: &str, method: &Method) -> Result<Arc<Route>, Miss> {
        self.read().find(path, query, method)
    }

    pub fn get(&self, id: u64) -> Option<Arc<Route>> {
        self.read().get(id).cloned()
    }

    /// The registrations of a path, looked up by the path they were registered with.
    pub fn registered_on(&self, path: &str) -> Vec<Arc<Route>> {
        self.read()
            .registered_on(path)
            .cloned()
            .collect()
    }

    /// The registrations of a path that answer at least one of the given methods.
    pub fn overlapping(&self, path: &str, methods: &[String]) -> Vec<Arc<Route>> {
        self.read()
            .overlapping(path, methods)
            .cloned()
            .collect()
    }

    /// Registers an endpoint. See [`EndpointStore::insert`] for what it replaces.
    pub fn insert(&self, path: String, endpoint: Endpoint, target: Option<u64>) -> Inserted {
        self.write().insert(path, endpoint, target)
    }

    pub fn remove(&self, id: u64) -> Option<Arc<Route>> {
        self.write().remove(id)
    }

    /// Removes every registration of a path, whatever its methods.
    pub fn remove_path(&self, path: &str) -> Vec<Arc<Route>> {
        self.write().remove_path(path)
    }

    pub fn has_kind(&self, kind: EndpointKind) -> bool {
        self.read()
            .iter()
            .any(|route| route.endpoint.kind() == kind)
    }

    pub fn len(&self) -> usize {
        self.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.read().is_empty()
    }

    /// Serializes every endpoint as a JSON list, in the order they were registered.
    pub fn to_json(&self) -> Vec<u8> {
        // Serializing cannot fail: the keys are strings and every value is plain data.
        serde_json::to_vec(&*self.read()).unwrap_or_default()
    }

    // A poisoned lock only means another request panicked. The store itself is still valid.
    fn read(&self) -> RwLockReadGuard<'_, EndpointStore> {
        self.store.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> RwLockWriteGuard<'_, EndpointStore> {
        self.store.write().unwrap_or_else(PoisonError::into_inner)
    }
}

/// An endpoint as the dashboard lists it: its settings, with the id and path it answers on.
#[derive(Serialize)]
struct Listed<'route> {
    id: u64,
    path: &'route str,
    #[serde(flatten)]
    endpoint: &'route Endpoint,
}

/// Serializes as a list of endpoints, the shape the dashboard lists.
impl Serialize for EndpointStore {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.routes
            .values()
            .map(|route| Listed { id: route.id, path: &route.path, endpoint: &route.endpoint }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use crate::models::MockBody;

    fn endpoint(file: &str, methods: &[&str]) -> Endpoint {
        Endpoint {
            body: MockBody::Raw(Bytes::new()),
            method: methods
                .iter()
                .map(|method| method.to_string())
                .collect(),
            group: None,
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
            store.insert(path.to_string(), endpoint(path, &["GET"]), None);
        }

        store
    }

    fn found(store: &EndpointStore, path: &str, query: &str) -> Option<String> {
        store.find(path, query, &Method::GET)
            .ok()
            .map(|route| route.path.clone())
    }

    /// The file of the route that answers, which tells registrations of one path apart.
    fn answered_by(store: &EndpointStore, method: Method, path: &str) -> Result<String, Miss> {
        store.find(path, "", &method).map(|route| route.endpoint.file.clone())
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
    fn registering_a_path_again_for_the_same_method_replaces_it_and_keeps_its_id() {
        let mut store = store_with(&["/api/hello"]);
        let first_id = store.find("/api/hello", "", &Method::GET).unwrap().id;

        let inserted = store.insert("/api/hello".to_string(), endpoint("second", &["GET"]), None);

        assert_eq!(inserted.id, first_id);
        assert_eq!(inserted.replaced.len(), 1);
        assert_eq!(inserted.replaced[0].endpoint.file, "/api/hello");
        assert_eq!(store.len(), 1);
        assert_eq!(answered_by(&store, Method::GET, "/api/hello"), Ok("second".to_string()));
    }

    #[test]
    fn one_literal_path_answers_each_method_with_its_own_registration() {
        let mut store = EndpointStore::default();
        store.insert("/users".to_string(), endpoint("list", &["GET"]), None);
        let inserted = store.insert("/users".to_string(), endpoint("create", &["POST"]), None);

        assert!(inserted.replaced.is_empty());
        assert_eq!(store.len(), 2);
        assert_eq!(answered_by(&store, Method::GET, "/users"), Ok("list".to_string()));
        assert_eq!(answered_by(&store, Method::POST, "/users"), Ok("create".to_string()));
    }

    #[test]
    fn one_templated_path_answers_each_method_with_its_own_registration() {
        let mut store = EndpointStore::default();
        store.insert("/users/{id}".to_string(), endpoint("read", &["GET"]), None);
        store.insert("/users/{id}".to_string(), endpoint("remove", &["DELETE"]), None);

        assert_eq!(answered_by(&store, Method::GET, "/users/7"), Ok("read".to_string()));
        assert_eq!(answered_by(&store, Method::DELETE, "/users/7"), Ok("remove".to_string()));
    }

    #[test]
    fn method_nobody_registered_is_refused_and_an_unknown_path_is_not_found() {
        let mut store = EndpointStore::default();
        store.insert("/users".to_string(), endpoint("list", &["GET"]), None);
        store.insert("/users/{id}".to_string(), endpoint("read", &["GET"]), None);

        assert_eq!(answered_by(&store, Method::PUT, "/users"), Err(Miss::MethodNotAllowed));
        assert_eq!(answered_by(&store, Method::PUT, "/users/7"), Err(Miss::MethodNotAllowed));
        assert_eq!(answered_by(&store, Method::GET, "/orders"), Err(Miss::NoRoute));
    }

    #[test]
    fn template_answers_a_method_the_literal_path_does_not_allow() {
        let mut store = EndpointStore::default();
        store.insert("/users/me".to_string(), endpoint("me", &["GET"]), None);
        store.insert("/users/{id}".to_string(), endpoint("remove", &["DELETE"]), None);

        assert_eq!(answered_by(&store, Method::GET, "/users/me"), Ok("me".to_string()));
        assert_eq!(answered_by(&store, Method::DELETE, "/users/me"), Ok("remove".to_string()));
    }

    #[test]
    fn registration_replaces_every_one_of_its_path_that_shares_a_method() {
        let mut store = EndpointStore::default();
        store.insert("/users".to_string(), endpoint("list", &["GET"]), None);
        store.insert("/users".to_string(), endpoint("create", &["POST"]), None);
        store.insert("/users".to_string(), endpoint("remove", &["DELETE"]), None);

        let inserted = store.insert("/users".to_string(), endpoint("both", &["GET", "POST"]), None);

        assert_eq!(inserted.replaced.len(), 2);
        assert_eq!(store.len(), 2);
        assert_eq!(answered_by(&store, Method::GET, "/users"), Ok("both".to_string()));
        assert_eq!(answered_by(&store, Method::POST, "/users"), Ok("both".to_string()));
        assert_eq!(answered_by(&store, Method::DELETE, "/users"), Ok("remove".to_string()));
    }

    #[test]
    fn target_is_replaced_even_when_its_methods_change() {
        let mut store = EndpointStore::default();
        let target = store.insert("/users".to_string(), endpoint("list", &["GET"]), None).id;

        let inserted = store.insert("/users".to_string(), endpoint("update", &["PUT"]), Some(target));

        assert_eq!(inserted.id, target);
        assert_eq!(store.len(), 1);
        assert_eq!(answered_by(&store, Method::PUT, "/users"), Ok("update".to_string()));
        assert_eq!(answered_by(&store, Method::GET, "/users"), Err(Miss::MethodNotAllowed));
    }

    #[test]
    fn ids_are_not_reused_after_a_removal() {
        let mut store = EndpointStore::default();
        let first = store.insert("/first".to_string(), endpoint("first", &["GET"]), None).id;
        store.remove(first);

        let second = store.insert("/second".to_string(), endpoint("second", &["GET"]), None).id;

        assert_ne!(first, second);
    }

    #[test]
    fn id_given_back_to_a_restored_endpoint_is_not_handed_out_again() {
        let mut store = EndpointStore::default();

        let restored = store.insert("/restored".to_string(), endpoint("restored", &["GET"]), Some(7));
        let earlier = store.insert("/earlier".to_string(), endpoint("earlier", &["GET"]), Some(3));
        let next = store.insert("/next".to_string(), endpoint("next", &["GET"]), None);

        assert_eq!(restored.id, 7);
        assert_eq!(earlier.id, 3);
        assert_eq!(next.id, 8);
    }

    #[test]
    fn removing_one_registration_leaves_the_others_of_its_path() {
        let mut store = EndpointStore::default();
        let list = store.insert("/users".to_string(), endpoint("list", &["GET"]), None).id;
        store.insert("/users".to_string(), endpoint("create", &["POST"]), None);

        store.remove(list);

        assert_eq!(answered_by(&store, Method::GET, "/users"), Err(Miss::MethodNotAllowed));
        assert_eq!(answered_by(&store, Method::POST, "/users"), Ok("create".to_string()));
    }

    #[test]
    fn removing_a_path_removes_all_its_registrations() {
        let mut store = store_with(&["/api/hello", "/api/user/{id}"]);
        store.insert("/api/hello".to_string(), endpoint("create", &["POST"]), None);

        assert_eq!(store.remove_path("/api/hello").len(), 2);
        assert_eq!(store.remove_path("/api/user/{id}").len(), 1);

        assert!(store.is_empty());
        assert_eq!(found(&store, "/api/hello", ""), None);
        assert_eq!(found(&store, "/api/user/42", ""), None);
    }

    #[test]
    fn lists_the_endpoints_in_the_order_they_were_registered() {
        let store = store_with(&["/second", "/first"]);

        let listed: serde_json::Value = serde_json::to_value(&store).unwrap();

        assert_eq!(listed[0]["path"], "/second");
        assert_eq!(listed[0]["id"], 1);
        assert_eq!(listed[0]["method"][0], "GET");
        assert_eq!(listed[1]["path"], "/first");
    }
}
