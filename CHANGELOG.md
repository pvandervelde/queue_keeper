# Changelog

All notable changes to this project will be documented in this file.

## [0.1.1] - 2026-02-22

### Bug Fixes

- Fix changelog extraction and add docker image link to release notes
- Fix release notes changelog extraction and add docker image link (#152)

## [0.1.0] - 2026-02-20

### Bug Fixes

- Update azure-sdk-for-rust monorepo
- Update azure-sdk-for-rust monorepo (#8)
- Update rust crate config to 0.15
- Update rust crate config to 0.15 (#11)
- Update rust crate tower-http to 0.6
- Update rust crate tower-http to 0.6 (#16)
- Update rust crate dirs to v6
- Update rust crate dirs to v6 (#18)
- Update rust crate jsonwebtoken to v10
- Update rust crate jsonwebtoken to v10 (#19)
- Implement graceful fallback for JWT cache read errors
- Implement graceful fallback for installation token cache read errors
- Fixing unit tests
- More unit test compilation errors
- Apply cargo fmt to resolve formatting issues
- Resolve protobuf vulnerability by upgrading opentelemetry to 0.29 (auto via agent)
- Add cargo-audit configuration to ignore known acceptable advisories (auto via agent)
- Update rust crate toml to 0.9
- Update rust crate toml to 0.9 (#14)
- Rust formatting
- Correct milestone handling in set_*_milestone operations
- Return AuthorizationFailed for non-rate-limit 403 errors
- Correct doc test examples in events module
- Fixing cargo format issues
- Remove duplicate delivery count increment in session abandon (auto via agent)
- Remove unmaintained json5 dependency from config crate
- Update DefaultWebhookProcessor instantiation
- Rust formatting issues
- Extract timestamp from ULID for accurate blob storage partitioning
- Rust formatting
- Address PR review comments - improve error handling and UUID validation
- Resolve integration test compilation errors
- Resolve CI failures and copilot PR comments
- Remove incomplete test files and fix imports (auto via agent)
- Resolve compilation errors in memory provider and e2e tests
- Add allow dead_code attributes for unused test infrastructure code
- Update integration test schemas to match refactored API
- Correct integration test assertions and expectations
- Fix integration test assertions and remove unused import
- Upgrade to Debian bookworm for GLIBC 2.36+ compatibility
- Change api_error_format test to use non-existent route
- Add allow(dead_code) to unused mock functions in common module
- Mark health_check_under_load as ignored performance test
- Remove redundant clone() calls in blob storage tests (auto via agent)
- Simplify string prefix check and add Default derive (auto via agent)
- Remove redundant clone on Copy type EventId (auto via agent)
- Add allow for complex mock type and remove redundant clones (auto via agent)
- Allow dead_code for contract test helper functions (auto via agent)
- Explicitly ignore unused must_use result in concurrent test (auto via agent)
- Remove redundant to_string in format macro (auto via agent)
- Simplify formatting and conditionals in queue-keeper-api (auto via agent)
- Remove unnecessary reference in CLI test (auto via agent)
- Simplify error construction in Azure Key Vault adapter (auto via agent)
- Remove redundant clone() calls in filesystem storage (auto via agent)
- Improve formatting in memory Key Vault adapter (auto via agent)
- Add allow for too_many_arguments in audit logging (auto via agent)
- Clean up DLQ storage test formatting (auto via agent)
- Clean up formatting and simplify code in github-bot-sdk (auto via agent)
- Correct Azure SDK imports and add futures dependency (auto via agent)
- Update to Azure SDK v0.21 compatible API
- Address code clean-up changes
- Address PR review comments (auto via agent)
- Resolve compilation errors and warnings in audit logging integration
- Allow dead_code for MockAuditLogger in integration tests
- Address PR review comments
- Remove duplicate processing_time assignment in webhook audit logging
- Address PR comments - add test isolation and remove hardcoded regions
- Resolve all clippy warnings across workspace (auto via agent)
- Correct version validation logic and improve workflow robustness (auto via agent)
- Update Dockerfile and cargo-deny for external dependencies (auto via agent)
- Allow OpenSSL license and skip wildcard checks for git dependencies (auto via agent)
- Allow wildcards for git dependencies in cargo-deny (auto via agent)
- Remove invalid allow-wildcard-paths array from deny.toml (auto via agent)
- Handle missing labels gracefully in release workflow
- Handle missing labels gracefully in release workflow (#144)
- Improve changelog generation robustness
- Use GitHub API directly for PR label check
- Use GitHub API directly for PR label check in release-comment workflow (#151)

### Build

- Add containerization infrastructure and CI validation (#107)

### CI/CD

- Adding the workflows
- Fixing issues
- Add GitHub Actions workflows for CI/CD pipeline (#47)
- Fix cargo deny errors
- Fix docker build failures
- Fix the git cliff failure
- Adding the workflow for Claude PR reviews
- Add claude automated pull request review workflow (#140)
- Fix workflow issues with repository context and validation (auto via agent)
- Fix workflow issues and add version override support (#143)

### Documentation

- Add comprehensive functional requirements specification
- Add queue runtime architecture specifications
- Add cloud provider implementation specifications
- Add observability and retry strategy specifications
- Add GitHub bot SDK specifications
- Add rustdoc comments to all test functions and fix type safety
- Fix assertion numbering sequence
- Update AuthenticationProvider trait to match implementation
- Correct JWT caching documentation
- Fix RFC reference and remove duplicate entries in shared registry
- Add test coverage tracking and task reminders
- Fix AWS provider doc test import path
- Add comprehensive release and contribution documentation (auto via agent)
- Add comprehensive configuration guide
- Convert repository_filter examples to snake_case (auto via agent)
- Update repository_filter examples to use YAML tag format (auto via agent)
- Add comprehensive configuration guide with repository filtering examples (#139)

### Features

- Add GitHub Bot SDK functional specification
- Add comprehensive Queue-Keeper webhook service specification
- Add queue runtime testing specification
- Add retry strategies specification for queue runtime
- Add core queue runtime module specifications
- Add Priority 1 critical architectural documents
- Add Priority 2 architectural specifications
- Add top 3 high-impact specifications for production readiness
- Add comprehensive Queue-Keeper architectural specifications and supporting runtime components (#1)
- Complete architectural specifications for all three systems
- Complete architectural specifications for all three systems (#2)
- Complete interface design phase
- Complete interface design phase (#3)
- Add core authentication types and agent guidelines (#43)
- Implement JWT generation for GitHub App authentication (#44)
- Implement token management for GitHub Bot SDK (#46)
- Implement app-level api client with rate limiting (#52)
- Update dependencies to latest versions and consolidate to workspace
- Update dependencies to latest versions and consolidate to workspace (#57)
- Add installation client interface specifications
- Add core type stubs (repository, issue)
- Add pull request type stubs
- Add workflow and release type stubs
- Add GitHub Projects v2 type stubs
- Add pagination and retry utility stubs
- Integrate installation client modules
- Add installation client base implementation stub
- Add installation client interface design and stubs (#58)
- Add installation-scoped client foundation (#59)
- Add repository and issue management operations (#64)
- Implement pull request and review operations (#66)
- Add pagination support for list operations (#67)
- Add rate limiting and retry logic for installation client (#68)
- Implement hmac-sha256 webhook signature validation (#71)
- Implement github webhook event types and processing (#72)
- Implement SendOptions and ReceiveOptions
- Add comprehensive tests for domain identifier types
- Implement core types with modular structure and comprehensive tests (#75)
- Implement client interface with factory pattern (#76)
- Implement in-memory queue provider with session support (#78)
- Implement session management for ordered message processing (#92)
- Implement core webhook processing pipeline (#93)
- Implement bot configuration and event routing engine (#96)
- Add event routing layer with session-based delivery (#99)
- Implement blob storage for webhook audit trail (#100)
- Implement HTTP webhook service with queue delivery and DLQ (#105)
- Add integration test infrastructure and initial test suites
- Add comprehensive unit tests for queue-keeper-api modules (auto via agent)
- Implement Key Vault secret management with Azure and in-memory providers (#110)
- Implement production-grade batch operations with JSON API and comprehensive error handling
- Implement azure service bus provider with http rest api (#118)
- Add circuit breaker wrappers for GitHub API and Queue operations (auto via agent)
- Full production-level circuit breaker implementation (auto via agent)
- Implement circuit breaker pattern for external services (#119)
- Implement comprehensive audit logging system for compliance (#126)
- Implement prometheus metrics collection and exposition (#127)
- Add secure Azure production configuration management (#130)
- Implement AWS Signature V4 signing and HTTP client setup
- Implement GetQueueUrl with HTTP and stub remaining operations
- Implement SendMessage operation with HTTP
- Add IAM role support for AWS SQS provider
- Implement AWS SQS provider with FIFO and session support (#131)
- Add release PR workflow with automatic changelog generation
- Add version suggestion workflow for release PRs
- Implement automated release workflow with version management (#134)
- Add webhook handler with signature validation (#135)
- Add snake_case serialization for RepositoryFilter enum (auto via agent)
- Add snake_case serialization for RepositoryFilter enum (#142)
- Generate and commit CHANGELOG.md in release workflow
- Generate and commit CHANGELOG.md in release workflow (#146)

### Miscellaneous

- Configure Renovate (#4)
- Cargo formatting
- Mark unimplemented tests as ignored and allow CDLA-Permissive-2.0 license
- Add branching rules to AGENTS.md
- Fix Rust formatting
- Upgrade dependencies (thiserror 2.0, rand 0.9, and consolidate workspace deps)
- Upgrade dependencies (thiserror 2.0, rand 0.9, and consolidate workspace deps) (#85)
- Update cargo.lock
- Rust formatting
- Remove unused RoutingContext struct
- Addressing PR comments
- Rust formatting
- Addressing PR comments
- Fix Rust formatting
- Fix cargo deny issues
- Remove the specs for the github-bot-sdk library
- Remove the specs for the queue-runtime library
- Update Cargo.lock with external dependencies
- Remove extracted crate directories

### Refactoring

- Moving tests into their own files
- Extract common request logic and improve error handling
- Reorganize module structure with domain-specific files
- Extract queue-keeper-api library and create test infrastructure
- Migrate github-bot-sdk and queue-runtime to external repositories
- Migrate github-bot-sdk and queue-runtime to external repositories (#138)

### Styling

- Remove redundant closures in error mapping

### Testing

- Implement three-tier testing architecture with docker e2e tests (#108)
- Add comprehensive test coverage for circuit breaker service wrappers

<!-- generated by git-cliff -->
