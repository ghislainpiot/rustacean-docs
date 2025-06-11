use thiserror::Error;

/// MCP protocol-related errors
#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("MCP protocol error: {message}")]
    Protocol { message: String },

    #[error("Invalid tool input: {tool_name} - {reason}")]
    InvalidInput { tool_name: String, reason: String },

    #[error("Tool not found: {tool_name}")]
    ToolNotFound { tool_name: String },

    #[error("Operation not supported: {operation}")]
    NotSupported { operation: String },

    #[error("Invalid request format: {details}")]
    InvalidRequest { details: String },

    #[error("Response format error: {details}")]
    ResponseFormat { details: String },
}

impl ProtocolError {
    pub fn protocol(message: impl Into<String>) -> Self {
        Self::Protocol {
            message: message.into(),
        }
    }

    pub fn invalid_input(tool_name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidInput {
            tool_name: tool_name.into(),
            reason: reason.into(),
        }
    }

    pub fn tool_not_found(tool_name: impl Into<String>) -> Self {
        Self::ToolNotFound {
            tool_name: tool_name.into(),
        }
    }

    pub fn not_supported(operation: impl Into<String>) -> Self {
        Self::NotSupported {
            operation: operation.into(),
        }
    }

    pub fn invalid_request(details: impl Into<String>) -> Self {
        Self::InvalidRequest {
            details: details.into(),
        }
    }

    pub fn response_format(details: impl Into<String>) -> Self {
        Self::ResponseFormat {
            details: details.into(),
        }
    }
}
