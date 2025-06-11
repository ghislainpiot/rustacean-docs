use thiserror::Error;

/// Configuration-related errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid configuration: {field} - {reason}")]
    InvalidField { field: String, reason: String },

    #[error("Missing required configuration: {field}")]
    MissingField { field: String },

    #[error("Configuration value out of range: {field} = {value} (expected {expected})")]
    OutOfRange {
        field: String,
        value: String,
        expected: String,
    },

    #[error("Configuration file not found: {path}")]
    FileNotFound { path: String },

    #[error("Failed to parse configuration file: {reason}")]
    ParseError { reason: String },

    #[error("Environment variable not set: {var_name}")]
    MissingEnvVar { var_name: String },
}

impl ConfigError {
    pub fn invalid_field(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidField {
            field: field.into(),
            reason: reason.into(),
        }
    }

    pub fn missing_field(field: impl Into<String>) -> Self {
        Self::MissingField {
            field: field.into(),
        }
    }

    pub fn out_of_range(
        field: impl Into<String>,
        value: impl Into<String>,
        expected: impl Into<String>,
    ) -> Self {
        Self::OutOfRange {
            field: field.into(),
            value: value.into(),
            expected: expected.into(),
        }
    }

    pub fn file_not_found(path: impl Into<String>) -> Self {
        Self::FileNotFound { path: path.into() }
    }

    pub fn parse_error(reason: impl Into<String>) -> Self {
        Self::ParseError {
            reason: reason.into(),
        }
    }

    pub fn missing_env_var(var_name: impl Into<String>) -> Self {
        Self::MissingEnvVar {
            var_name: var_name.into(),
        }
    }
}
