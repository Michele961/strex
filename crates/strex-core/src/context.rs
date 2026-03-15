use std::collections::HashMap;

use crate::collection::Collection;

/// Merged variable context for a single collection execution.
///
/// Three layers are merged by [`resolve_all`] in priority order (highest first):
/// `data` > `variables` > `environment`.
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Inline environment block from the collection — lowest priority in variable resolution.
    pub environment: HashMap<String, String>,
    /// Mutable per-iteration collection variables — mid priority.
    pub variables: HashMap<String, String>,
    /// Per-iteration data row — highest priority. Populated by [`ExecutionContext::new_with_data`]
    /// for data-driven runs; empty for standard single-collection runs.
    pub data: HashMap<String, String>,
}

impl ExecutionContext {
    /// Create a fresh context from a parsed collection.
    ///
    /// `Collection::variables` entries with `None` values (declared as `null` in YAML)
    /// are converted to empty strings — intentional; `None` means "declared but not yet set".
    pub fn new(collection: &Collection) -> Self {
        let variables = collection
            .variables
            .iter()
            .map(|(k, v)| (k.clone(), v.clone().unwrap_or_default()))
            .collect();

        Self {
            environment: collection.environment.clone(),
            variables,
            data: HashMap::new(),
        }
    }

    /// Create a context pre-populated with a data row.
    ///
    /// `data` values shadow `variables` and `environment` in [`resolve_all`],
    /// so each row's values take highest priority during template interpolation.
    pub fn new_with_data(collection: &Collection, row: &HashMap<String, String>) -> Self {
        let mut ctx = Self::new(collection);
        ctx.data = row.clone();
        ctx
    }

    /// Merge all three layers into a flat `HashMap` for use with [`interpolate`].
    ///
    /// Keys in higher-priority layers shadow the same key in lower-priority layers.
    /// No prefixes — all variables are accessed as bare names (e.g. `{{baseUrl}}`).
    pub fn resolve_all(&self) -> HashMap<String, String> {
        let mut merged = self.environment.clone();
        merged.extend(self.variables.clone());
        merged.extend(self.data.clone());
        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collection_with(
        environment: HashMap<String, String>,
        variables: HashMap<String, Option<String>>,
    ) -> Collection {
        Collection {
            name: "test".to_string(),
            version: "1.0".to_string(),
            environment,
            variables,
            requests: vec![],
        }
    }

    #[test]
    fn data_beats_variables_beats_environment() {
        let env = HashMap::from([("key".to_string(), "from_env".to_string())]);
        let vars = HashMap::from([("key".to_string(), Some("from_vars".to_string()))]);
        let col = collection_with(env, vars);
        let mut ctx = ExecutionContext::new(&col);
        ctx.data.insert("key".to_string(), "from_data".to_string());

        assert_eq!(ctx.resolve_all()["key"], "from_data");
    }

    #[test]
    fn variables_beat_environment() {
        let env = HashMap::from([("key".to_string(), "from_env".to_string())]);
        let vars = HashMap::from([("key".to_string(), Some("from_vars".to_string()))]);
        let col = collection_with(env, vars);
        let ctx = ExecutionContext::new(&col);

        assert_eq!(ctx.resolve_all()["key"], "from_vars");
    }

    #[test]
    fn environment_used_when_no_override() {
        let env = HashMap::from([("baseUrl".to_string(), "https://api.example.com".to_string())]);
        let col = collection_with(env, HashMap::new());
        let ctx = ExecutionContext::new(&col);

        assert_eq!(ctx.resolve_all()["baseUrl"], "https://api.example.com");
    }

    #[test]
    fn none_variable_becomes_empty_string() {
        let vars = HashMap::from([("token".to_string(), None)]);
        let col = collection_with(HashMap::new(), vars);
        let ctx = ExecutionContext::new(&col);

        assert_eq!(ctx.variables["token"], "");
        assert_eq!(ctx.resolve_all()["token"], "");
    }

    #[test]
    fn key_absent_from_all_layers_not_in_resolved() {
        let col = collection_with(HashMap::new(), HashMap::new());
        let ctx = ExecutionContext::new(&col);

        assert!(!ctx.resolve_all().contains_key("missing"));
    }

    #[test]
    fn data_field_starts_empty() {
        let col = collection_with(HashMap::new(), HashMap::new());
        let ctx = ExecutionContext::new(&col);
        assert!(ctx.data.is_empty());
    }

    #[test]
    fn new_with_data_populates_data_field() {
        let col = collection_with(HashMap::new(), HashMap::new());
        let row = HashMap::from([
            ("email".to_string(), "alice@example.com".to_string()),
            ("name".to_string(), "Alice".to_string()),
        ]);
        let ctx = ExecutionContext::new_with_data(&col, &row);
        assert_eq!(ctx.data["email"], "alice@example.com");
        assert_eq!(ctx.data["name"], "Alice");
    }

    #[test]
    fn new_with_data_row_beats_collection_variable() {
        let vars = HashMap::from([("key".to_string(), Some("from_vars".to_string()))]);
        let col = collection_with(HashMap::new(), vars);
        let row = HashMap::from([("key".to_string(), "from_row".to_string())]);
        let ctx = ExecutionContext::new_with_data(&col, &row);
        assert_eq!(ctx.resolve_all()["key"], "from_row");
    }
}
