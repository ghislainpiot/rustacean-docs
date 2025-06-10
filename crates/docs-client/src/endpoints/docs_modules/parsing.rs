// This module will contain the HTML parsing functions extracted from docs.rs
// For now, I'll add public re-exports of the functions that are still in docs.rs

pub use super::{
    parse_crate_documentation, parse_item_documentation, parse_recent_releases,
    resolve_item_path_with_fallback,
};