//! Provides JSON parsing utilities for model formats.
//!
//! Common helpers for extracting data from serde_json::Value types,
//! used by Blockbench and potentially other JSON-based formats.
//!
//! # Examples
//! ```
//! use serde_json::json;
//!
//! use glimpse::formats::shared::json_str_or_none;
//!
//! assert_eq!(json_str_or_none(&json!("hello")), Some("hello"));
//! ```

use crate::formats::Vec3;

/// Extracts a string from a JSON value, returning None if empty.
///
/// # Examples
/// ```
/// use serde_json::json;
///
/// use glimpse::formats::shared::json_str_or_none;
///
/// assert_eq!(json_str_or_none(&json!("value")), Some("value"));
/// assert_eq!(json_str_or_none(&json!("")), None);
/// ```
pub fn json_str_or_none(value: &serde_json::Value) -> Option<&str> {
    value.as_str().filter(|s| !s.is_empty())
}

/// Parses a JSON array as a Vec3 [x, y, z].
///
/// # Examples
/// ```
/// use serde_json::json;
///
/// use glimpse::formats::shared::parse_vec3;
///
/// assert_eq!(parse_vec3(&json!([1, 2, 3])), Some([1.0, 2.0, 3.0]));
/// ```
pub fn parse_vec3(value: &serde_json::Value) -> Option<Vec3> {
    let arr = value.as_array()?;
    if arr.len() < 3 {
        return None;
    }
    Some([
        arr[0].as_f64().unwrap_or(0.0) as f32,
        arr[1].as_f64().unwrap_or(0.0) as f32,
        arr[2].as_f64().unwrap_or(0.0) as f32,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_str_or_none() {
        assert_eq!(json_str_or_none(&json!("hello")), Some("hello"));
        assert_eq!(json_str_or_none(&json!("")), None);
        assert_eq!(json_str_or_none(&json!(null)), None);
        assert_eq!(json_str_or_none(&json!(123)), None);
    }

    #[test]
    fn test_parse_vec3() {
        assert_eq!(parse_vec3(&json!([1.0, 2.0, 3.0])), Some([1.0, 2.0, 3.0]));
        assert_eq!(parse_vec3(&json!([1, 2, 3])), Some([1.0, 2.0, 3.0]));
        assert_eq!(parse_vec3(&json!([1.5, -2.5, 0.0])), Some([1.5, -2.5, 0.0]));
        assert_eq!(parse_vec3(&json!([1, 2])), None); // Too short
        assert_eq!(parse_vec3(&json!("not an array")), None);
        assert_eq!(parse_vec3(&json!(null)), None);
    }
}
