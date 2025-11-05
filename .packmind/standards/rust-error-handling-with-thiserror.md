# Rust Error Handling with thiserror

This standard defines error handling patterns using the `thiserror` crate for creating well-structured, maintainable custom error types. Apply these rules when defining new error types or refactoring existing error handling code in this Rust codebase.

## Rules
      
* Use `thiserror` with `#[derive(Error, Debug)]` for all custom error types to automatically implement the Error trait and provide Debug formatting.
* Define domain-specific error enums that group related error cases together rather than creating a single catch-all error type.
* Use `#[error(transparent)]` with `#[from]` when wrapping lower-level errors to preserve the original error chain and enable automatic conversion.
* Write clear, descriptive error messages using the `#[error("...")]` attribute with format placeholders to include relevant context in the error display.
* Create a top-level Error enum that aggregates all domain-specific errors using transparent wrapping to provide a unified error type for the entire crate.
* Define a type alias `pub type Result<T> = std::result::Result<T, Error>` to reduce boilerplate when returning results throughout the crate.
