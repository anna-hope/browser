use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use thiserror::Error;

pub const USER_AGENT: &str = "Octo";

#[derive(Debug, Error)]
pub enum HeadersError {
    #[error("Expected exactly 1 value, got {0}")]
    NotOneValue(usize),
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Headers {
    headers: HashMap<String, Vec<String>>,
}

impl Headers {
    pub(crate) fn add_header_values(&mut self, key: &str, values: &[&str]) {
        let key = key.to_lowercase();

        // TODO: Check the spec to make sure we actually want to filter out empty strings
        // from values here.
        let values = values
            .iter()
            .filter_map(|s| {
                if !s.is_empty() {
                    Some(s.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if let Some(existing_values) = self.headers.get_mut(&key) {
            for value in values {
                existing_values.push(value.to_string());
            }
        } else {
            self.headers.insert(key, values);
        }
    }

    pub(crate) fn add_many(&mut self, kv_pairs: &[(&str, &[&str])]) {
        for (key, values) in kv_pairs {
            self.add_header_values(key, values);
        }
    }

    /// Adds one header key/value pair, where the value is a single header value.
    /// Example: `add_one_pair("content-encoding", "gzip")`
    /// **Note:** The header **key** will be converted to lowercase, but the **value** will not.
    /// Does not perform any deduplication.
    pub(crate) fn add(&mut self, key: &str, value: &str) {
        self.add_header_values(key, &[value]);
    }

    #[inline]
    pub fn get(&self, key: &str) -> Option<&Vec<String>> {
        self.headers.get(key)
    }

    /// Convenience method to get the first value of a header,
    /// since many headers have only one value.
    /// Returns `Some(Ok(value))` if the header exists and has only one value,
    /// `Some(Err(HeadersError::NotOneValue))` if the header has 0 or more than 1 value,
    /// Or `None` if the header doesn't exist.
    ///
    /// This return signature is kind of complicated,
    /// but I don't want to shoot myself in the foot by making the wrong assumptions
    /// and getting a value that I don't expect.
    #[allow(clippy::unwrap_used)]
    pub fn get_single_value(&self, key: &str) -> Option<anyhow::Result<&String, HeadersError>> {
        if let Some(values) = self.get(key) {
            return if values.len() == 1 {
                // Ok to unwrap since we're guaranteed to have a value.
                Some(Ok(values.first().unwrap()))
            } else {
                Some(Err(HeadersError::NotOneValue(values.len())))
            };
        }
        None
    }

    /// Returns `Some(true)` if the given header key is associated with the given value,
    /// `Some(false)` if the given header is not associated with the given value,
    /// and `None` if the given header is not in `Headers` at all.
    pub(crate) fn has_given_value(&self, key: &str, value: &str) -> Option<bool> {
        self.headers
            .get(key)
            .map(|values| values.iter().any(|s| s.as_str() == value))
    }

    pub(crate) fn from(kv_pairs: &[(&str, &[&str])]) -> Self {
        let mut headers = Headers::default();
        headers.add_many(kv_pairs);
        headers
    }
}

impl Display for Headers {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        for (key, values) in &self.headers {
            let values = values.join(", ");
            s.push_str(format!("{key}: {values}\r\n").as_str());
        }
        write!(f, "{s}")
    }
}
