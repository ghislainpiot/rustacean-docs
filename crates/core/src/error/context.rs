use super::{Error, Result};
use std::collections::HashMap;

/// Structured context for errors
#[derive(Debug, Clone)]
pub struct StructuredContext {
    fields: HashMap<String, String>,
}

impl StructuredContext {
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }

    pub fn add(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> String {
        self.fields
            .into_iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl Default for StructuredContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension trait for adding context to errors
pub trait ErrorContext<T> {
    /// Add context to an error
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;

    /// Add context to an error with a static string
    fn context(self, msg: &'static str) -> Result<T>;

    /// Add structured context to an error
    fn with_structured_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce(StructuredContext) -> StructuredContext;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: Into<Error>,
{
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let original_error = e.into();
            Error::Internal(format!("{}: {}", f(), original_error))
        })
    }

    fn context(self, msg: &'static str) -> Result<T> {
        self.with_context(|| msg.to_string())
    }

    fn with_structured_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce(StructuredContext) -> StructuredContext,
    {
        self.map_err(|e| {
            let context = f(StructuredContext::new());
            let original_error = e.into();
            Error::Internal(format!("{} [{}]", original_error, context.build()))
        })
    }
}

impl<T> ErrorContext<T> for Option<T> {
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| Error::Internal(f()))
    }

    fn context(self, msg: &'static str) -> Result<T> {
        self.with_context(|| msg.to_string())
    }

    fn with_structured_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce(StructuredContext) -> StructuredContext,
    {
        self.ok_or_else(|| {
            let context = f(StructuredContext::new());
            Error::Internal(format!("None value [{}]", context.build()))
        })
    }
}
