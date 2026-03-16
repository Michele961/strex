use std::collections::HashMap;

use crate::cli::ValidateArgs;

/// Execute the `validate` subcommand.
///
/// Parses the collection at `args.collection`, walks every interpolatable
/// string for `{{placeholder}}` references, and cross-references them
/// against `collection.variables` keys.
///
/// Returns `Ok(0)` when the collection is valid; bails with a descriptive
/// error (→ exit 2 via `main`) when an unresolved variable is found.
pub async fn execute(args: ValidateArgs) -> anyhow::Result<i32> {
    let collection = strex_core::parse_collection(&args.collection)?;

    let declared: std::collections::HashSet<&str> =
        collection.variables.keys().map(String::as_str).collect();

    // Walk requests in order: url → headers (sorted) → body → assertions.
    for (idx, request) in collection.requests.iter().enumerate() {
        // --- url ---
        check_unresolved(
            extract_placeholders(&request.url),
            &declared,
            &collection.variables,
            idx,
            "url",
        )?;

        // --- headers (sorted by key for determinism) ---
        let mut header_keys: Vec<&str> = request.headers.keys().map(String::as_str).collect();
        header_keys.sort_unstable();
        for key in header_keys {
            let value = &request.headers[key];
            check_unresolved(
                extract_placeholders(value),
                &declared,
                &collection.variables,
                idx,
                "headers",
            )?;
        }

        // --- body ---
        if let Some(body) = &request.body {
            let mut body_placeholders = Vec::new();
            walk_yaml_value(&body.content, &mut body_placeholders);
            check_unresolved(
                body_placeholders,
                &declared,
                &collection.variables,
                idx,
                "body",
            )?;
        }

        // --- assertions ---
        for assertion_map in &request.assertions {
            let mut assertion_placeholders = Vec::new();
            for value in assertion_map.values() {
                walk_yaml_value(value, &mut assertion_placeholders);
            }
            check_unresolved(
                assertion_placeholders,
                &declared,
                &collection.variables,
                idx,
                "assertions",
            )?;
        }
    }

    let filename = args
        .collection
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("collection");

    println!(
        "valid: {} ({} requests, 0 unresolved variables)",
        filename,
        collection.requests.len(),
    );

    Ok(0)
}

/// Check whether any placeholder in the list is undeclared, and bail if so.
fn check_unresolved(
    placeholders: Vec<String>,
    declared: &std::collections::HashSet<&str>,
    variables: &std::collections::HashMap<String, Option<String>>,
    req_idx: usize,
    field: &str,
) -> anyhow::Result<()> {
    for placeholder in placeholders {
        if !declared.contains(placeholder.as_str()) {
            let declared_list = sorted_declared_list(variables);
            anyhow::bail!(
                "error: unresolved variable `{placeholder}` in requests[{req_idx}].{field}\n  declared variables: {declared_list}"
            );
        }
    }
    Ok(())
}

/// Extract all `{{placeholder}}` names from a string.
fn extract_placeholders(s: &str) -> Vec<String> {
    let mut placeholders = Vec::new();
    let mut remaining = s;
    while let Some(start) = remaining.find("{{") {
        remaining = &remaining[start + 2..];
        if let Some(end) = remaining.find("}}") {
            let name = remaining[..end].trim().to_string();
            if !name.is_empty() {
                placeholders.push(name);
            }
            remaining = &remaining[end + 2..];
        } else {
            break;
        }
    }
    placeholders
}

/// Recursively walk a `serde_yaml::Value`, collecting placeholders from String leaves.
fn walk_yaml_value(value: &serde_yaml::Value, placeholders: &mut Vec<String>) {
    match value {
        serde_yaml::Value::String(s) => {
            placeholders.extend(extract_placeholders(s));
        }
        serde_yaml::Value::Sequence(seq) => {
            for item in seq {
                walk_yaml_value(item, placeholders);
            }
        }
        serde_yaml::Value::Mapping(map) => {
            for (_, v) in map {
                walk_yaml_value(v, placeholders);
            }
        }
        // Null, Bool, Number, Tagged — no string content to scan.
        _ => {}
    }
}

/// Build the sorted, comma-joined declared-variables string for error messages.
fn sorted_declared_list(variables: &HashMap<String, Option<String>>) -> String {
    if variables.is_empty() {
        return "(none)".to_string();
    }
    let mut keys: Vec<&str> = variables.keys().map(String::as_str).collect();
    keys.sort_unstable();
    keys.join(", ")
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
