# CLI Implementation Improvements Plan

This document outlines the implementation plan for fixing the HIGH and MEDIUM priority issues identified in the CLI vs Python MCP comparison.

## Overview

Based on the comprehensive comparison, we need to address:
1. **HIGH**: Complete CLI implementation (metadata, releases, version defaulting)
2. **MEDIUM**: URL format consistency 
3. **MEDIUM**: Version handling standardization

## Implementation Plan

### Phase 1: HIGH Priority - Complete CLI Implementation

#### Task 1.1: Implement Real get_crate_metadata Functionality

**Current State**: Returns mock data with placeholder values
**Target State**: Fetch real metadata from crates.io API

**Implementation Steps:**

1. **Add crates.io API client functionality**
   - File: `crates/docs-client/src/endpoints/metadata.rs` (create new)
   - Implement HTTP client calls to `https://crates.io/api/v1/crates/{crate_name}`
   - Add response parsing for crates.io metadata format

2. **Update data models**
   - File: `crates/core/src/models/metadata.rs`
   - Ensure models match crates.io API response structure
   - Add transformation logic from crates.io format to internal format

3. **Replace mock implementation**
   - File: `crates/mcp-server/src/tools/metadata.rs`
   - Remove mock data generation
   - Integrate real API calls via docs client

4. **Add error handling**
   - Handle API failures gracefully
   - Provide meaningful error messages
   - Add retry logic for transient failures

**API Integration Details:**
```
GET https://crates.io/api/v1/crates/{crate_name}
Response includes:
- crate metadata (name, description, license, repository, etc.)
- version information
- dependency lists (runtime, dev, build)
- download statistics
```

**Estimated Effort**: 2-3 days

#### Task 1.2: Implement Real list_recent_releases Functionality  

**Current State**: Returns single mock release entry
**Target State**: Fetch real recent releases from docs.rs or crates.io

**Implementation Steps:**

1. **Choose data source strategy**
   - Option A: Scrape docs.rs homepage (like Python MCP)
   - Option B: Use crates.io API with sorting by recent updates
   - **Recommendation**: Option B for reliability

2. **Implement crates.io recent releases API**
   - File: `crates/docs-client/src/endpoints/releases.rs` (create new) 
   - API endpoint: `https://crates.io/api/v1/crates?sort=recent-updates&per_page={limit}`
   - Parse response and extract relevant fields

3. **Update models and transformation**
   - File: `crates/core/src/models/docs.rs`
   - Ensure CrateRelease model matches API response
   - Transform crates.io format to internal format

4. **Replace mock implementation**
   - File: `crates/mcp-server/src/tools/releases.rs`
   - Remove hardcoded mock data
   - Integrate real API calls

**API Integration Details:**
```
GET https://crates.io/api/v1/crates?sort=recent-updates&per_page=20
Response includes:
- Array of recently updated crates
- Each crate has: name, newest_version, updated_at, description, downloads
```

**Estimated Effort**: 1-2 days

#### Task 1.3: Add Automatic Version Defaulting for get_item_docs

**Current State**: Requires explicit version parameter
**Target State**: Defaults to "latest" when version not specified

**Implementation Steps:**

1. **Update request handling**
   - File: `crates/docs-client/src/endpoints/docs.rs`
   - Modify `get_item_docs` to default version to "latest"
   - Update path construction logic

2. **Update MCP tool handler**
   - File: `crates/mcp-server/src/tools/item_docs.rs`
   - Make version parameter optional in request validation
   - Default to "latest" when not provided

3. **Update parameter schema**
   - Remove version from required parameters
   - Update documentation to reflect optional nature

**Code Changes Required:**
```rust
// In ItemDocsRequest::new()
pub fn new(crate_name: &str, item_path: &str) -> Self {
    Self {
        crate_name: crate_name.to_string(),
        item_path: item_path.to_string(),
        version: Some("latest".to_string()), // Default to latest
    }
}
```

**Estimated Effort**: 0.5 days

### Phase 2: MEDIUM Priority - URL Format Consistency

#### Task 2.1: Standardize on Versioned URLs

**Current State**: CLI generates `https://docs.rs/crate_name`
**Target State**: Generate `https://docs.rs/crate_name/latest/crate_name/`

**Implementation Steps:**

1. **Update search result URL generation**
   - File: `crates/docs-client/src/endpoints/search.rs`
   - Function: `transform_crate_data()`
   - Change docs_url construction to include version

2. **Update crate docs URL generation**
   - File: `crates/docs-client/src/endpoints/docs.rs`
   - Function: `parse_crate_documentation()`
   - Include version in docs_url construction

3. **Update item docs URL generation**
   - File: `crates/docs-client/src/endpoints/docs.rs`
   - Function: `parse_item_documentation()`
   - Ensure item URLs include full version path

**Code Changes Required:**
```rust
// Before:
let docs_url = Some(Url::parse(&format!("https://docs.rs/{}", crate_name))?);

// After:
let version = version.as_deref().unwrap_or("latest");
let docs_url = Some(Url::parse(&format!("https://docs.rs/{}/{}/{}/", crate_name, version, crate_name))?);
```

**Estimated Effort**: 1 day

### Phase 3: MEDIUM Priority - Version Handling Standardization

#### Task 3.1: Ensure Consistent Version Parameter Handling

**Current State**: Inconsistent version handling across endpoints
**Target State**: All endpoints default to "latest" consistently

**Implementation Steps:**

1. **Audit all endpoint version handling**
   - Review: `get_crate_docs`, `get_item_docs`, metadata endpoints
   - Ensure consistent defaulting behavior

2. **Create version utility functions**
   - File: `crates/core/src/utils/version.rs` (create new)
   - Common functions for version defaulting and validation

3. **Update all request constructors**
   - Standardize version defaulting across all request types
   - Use consistent version resolution logic

4. **Update MCP tool parameter schemas**
   - Make version optional across all tools where applicable
   - Update documentation consistently

**Common Version Utility:**
```rust
pub fn resolve_version(version: Option<String>) -> String {
    version.unwrap_or_else(|| "latest".to_string())
}

pub fn normalize_version(version: &str) -> &str {
    if version.is_empty() { "latest" } else { version }
}
```

**Estimated Effort**: 1 day

## Implementation Timeline

### Week 1: HIGH Priority Items
- **Days 1-3**: Task 1.1 - Implement get_crate_metadata
- **Days 4-5**: Task 1.2 - Implement list_recent_releases  
- **Day 5**: Task 1.3 - Add version defaulting

### Week 2: MEDIUM Priority Items
- **Day 1**: Task 2.1 - URL format consistency
- **Day 2**: Task 3.1 - Version handling standardization
- **Days 3-5**: Testing, integration, documentation

## Testing Strategy

### Unit Tests
- Test crates.io API integration with mock responses
- Test URL generation with various version inputs
- Test version defaulting logic

### Integration Tests  
- Test end-to-end metadata fetching
- Test recent releases retrieval
- Test version handling across all endpoints

### Comparison Tests
- Verify CLI output matches or exceeds Python MCP functionality
- Test URL format consistency
- Validate error handling improvements

## Files to Modify/Create

### New Files:
- `crates/docs-client/src/endpoints/metadata.rs`
- `crates/docs-client/src/endpoints/releases.rs`
- `crates/core/src/utils/version.rs`

### Modified Files:
- `crates/docs-client/src/endpoints/docs.rs`
- `crates/docs-client/src/endpoints/search.rs`
- `crates/mcp-server/src/tools/metadata.rs`
- `crates/mcp-server/src/tools/releases.rs`
- `crates/mcp-server/src/tools/item_docs.rs`
- `crates/core/src/models/metadata.rs`
- `crates/core/src/models/docs.rs`

## Dependencies Required

### New Dependencies:
```toml
# In docs-client/Cargo.toml
[dependencies]
# May need additional HTTP client features for crates.io API
reqwest = { version = "0.11", features = ["json", "gzip"] }
```

## Risk Assessment

### High Risk:
- **crates.io API rate limiting**: Need to implement respectful rate limiting
- **API response format changes**: Need robust error handling

### Medium Risk:
- **URL format breaking changes**: Need to verify docs.rs URL patterns remain stable
- **Version resolution edge cases**: Need comprehensive testing

### Low Risk:
- **Integration complexity**: Changes are mostly additive
- **Backward compatibility**: Existing functionality should remain intact

## Success Criteria

### Functional Requirements:
1. ✅ CLI get_crate_metadata returns real data from crates.io
2. ✅ CLI list_recent_releases returns real recent releases
3. ✅ CLI get_item_docs works without version parameter
4. ✅ All CLI URLs follow versioned format consistently
5. ✅ Version handling is consistent across all endpoints

### Quality Requirements:
1. ✅ Comprehensive test coverage for new functionality
2. ✅ Error handling for API failures
3. ✅ Performance similar to or better than Python MCP
4. ✅ Documentation updated to reflect changes
5. ✅ No regression in existing functionality

## Post-Implementation

### Monitoring:
- Track API call success rates
- Monitor response times
- Watch for crates.io API changes

### Future Enhancements:
- Add caching for crates.io API responses
- Implement background refresh for recent releases
- Add metadata caching with TTL

### Documentation Updates:
- Update CLI_vs_MCP_COMPARISON_PLAN.md with results
- Update documentation with new capabilities
- Add API integration documentation