use std::collections::HashMap;
use chrono::Utc;
use regex::Regex;

/// Replaces placeholders in the response body with actual values.
///
/// - `body`: The response template with placeholders.
/// - `params`: A map containing request parameters (e.g., query or path params).
///
/// Supports:
/// - `{{timestamp}}` → Current UTC timestamp
/// - Other `{{var}}` → Replaced with values from `params`
pub fn replace_variables(body: &str, params: &HashMap<String, String>) -> String {
    let rx = Regex::new(r"\{\{(\w+)}}").unwrap();
    
    rx.replace_all(body, |caps: &regex::Captures| {
        let key = &caps[1];
        
        match key {
            "timestamp" => Utc::now().to_rfc3339(),
            _ => params.get(key).cloned().unwrap_or_else(|| format!("{{{{{}}}}}", key)),
        }
    }).into_owned()
}