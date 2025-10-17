# Agent Development Guidelines

This document provides guidelines and conventions for AI agents working on this codebase.

## Code Organization

### Module Structure

Organize code into logical modules following clean architecture principles:

```
src/
├── lib.rs              # Public API and module declarations
├── error.rs            # Error types and handling
├── error_tests.rs      # Error tests
├── module1.rs          # Module 1 public interface
├── module1/
│   ├── types.rs        # Domain types
│   ├── types_tests.rs  # Type tests
│   └── implementation.rs
├── module2.rs          # Module 2 public interface
└── module2/
    └── ...
```

### Naming Conventions

- **Module names**: lowercase with underscores (`auth_provider`, `queue_client`)
- **Type names**: PascalCase (`GitHubAppId`, `InstallationToken`)
- **Function names**: snake_case (`get_installation_token`, `is_expired`)
- **Test files**: `<source_file>_tests.rs`
- **Test functions**: `test_<what_is_being_tested>`

## Documentation

### Rustdoc Requirements

All public APIs must have rustdoc comments:

```rust
/// Brief one-line summary of what this does.
///
/// More detailed explanation of the functionality, including:
/// - Key behaviors
/// - Important constraints
/// - Edge cases
///
/// # Examples
///
/// ```rust
/// use crate::MyType;
///
/// let instance = MyType::new(42);
/// assert_eq!(instance.value(), 42);
/// ```
///
/// # Errors
///
/// Returns `ErrorType` if:
/// - Condition 1
/// - Condition 2
pub fn public_api() -> Result<(), ErrorType> {
    // Implementation...
}
```

### Test Documentation

Test functions should have doc comments explaining what they verify:

```rust
/// Verify that expired tokens are correctly identified.
///
/// Creates a token that expired 5 minutes ago and verifies
/// that `is_expired()` returns true.
#[test]
fn test_token_expiration_detection() {
    // Test implementation...
}
```

## Error Handling

### Error Type Guidelines

1. Use `thiserror` for error type derivation
2. Implement retry classification (`is_transient()`, `should_retry()`)
3. Include sufficient context for debugging
4. Never expose secrets in error messages

```rust
#[derive(Debug, Error)]
pub enum MyError {
    #[error("Operation failed: {context}")]
    OperationFailed { context: String },

    #[error("Resource not found: {id}")]
    NotFound { id: String },
}

impl MyError {
    pub fn is_transient(&self) -> bool {
        match self {
            Self::OperationFailed { .. } => true,
            Self::NotFound { .. } => false,
        }
    }
}
```

## Testing Strategy

### Test Categories

1. **Unit Tests**: Test individual functions and types in isolation
2. **Integration Tests**: Test interactions between components
3. **Contract Tests**: Verify trait implementations meet contracts
4. **Property Tests**: Use `proptest` for property-based testing

### Test Coverage Expectations

- **Core Business Logic**: 100% coverage
- **Error Paths**: All error variants tested
- **Edge Cases**: Boundary conditions covered
- **Security-Critical Code**: Exhaustive testing (e.g., token handling, validation)

### Test File Organization

Tests should be organized in separate test files adjacent to the code they test, following this pattern:

**For a source file**: `src/module/file.rs`
**Create test file**: `src/module/file_tests.rs`

**For a module file**: `src/module/mod.rs`
**Create test file**: `src/module/mod_tests.rs`

### Test Module Declaration

Reference the external test file from the source file using:

```rust
#[cfg(test)]
#[path = "<TEST_FILE_NAME_WITH_EXTENSION>"]
mod tests;
```

### Test Organization Patterns

```rust
//! Tests for authentication module.

use super::*;

// Group related tests using module organization
mod token_tests {
    use super::*;

    #[test]
    fn test_token_creation() { }

    #[test]
    fn test_token_expiry() { }
}

mod validation_tests {
    use super::*;

    #[test]
    fn test_valid_input() { }

    #[test]
    fn test_invalid_input() { }
}
```

## Rust-Specific Conventions

### Type Safety

- Use newtype pattern for domain identifiers
- Leverage type system to prevent invalid states
- Use `#[must_use]` for types that shouldn't be ignored

```rust
/// GitHub App identifier.
///
/// This is a newtype wrapper to prevent mixing up different ID types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct GitHubAppId(u64);
```

### Async/Await

- All I/O operations must be async
- Use `#[async_trait]` for async trait methods
- Document cancellation behavior
- Ensure proper resource cleanup

### Security

- Never log secrets or tokens
- Implement `Debug` carefully for sensitive types
- Use constant-time comparison for security-critical operations
- Zero sensitive memory on drop when possible

```rust
impl std::fmt::Debug for SecretToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretToken")
            .field("token", &"<REDACTED>")
            .finish()
    }
}
```

## Commit Guidelines

When working as an automated agent:

1. **Atomic Commits**: Each commit should represent one logical change
2. **Descriptive Messages**: Use conventional commit format `<type>(<scope>): <description> (auto via agent)`
3. **Separate Concerns**: Tests and implementation in different commits when following TDD

## Dependencies

### Adding Dependencies

Before adding a dependency, verify:

1. Active maintenance and security track record
2. Compatible license (MIT/Apache-2.0 preferred)
3. Minimal transitive dependencies
4. Well-documented and tested
5. Rust-native when possible
6. Use workspace dependencies for shared crates

## Summary

Following these conventions ensures:

- **Consistency**: Codebase looks like one person wrote it
- **Maintainability**: Easy to find and understand code
- **Quality**: High test coverage and clear documentation
- **Security**: Sensitive data handled properly
- **Performance**: Conscious resource management

When in doubt, look at existing code in the repository as examples of these patterns in practice.
