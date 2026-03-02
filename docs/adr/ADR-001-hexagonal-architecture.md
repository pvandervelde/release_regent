# ADR-001: Hexagonal Architecture for ReleaseRegent

Status: Accepted
Date: 2026-01-31
Owners: ReleaseRegent team

## Context

ReleaseRegent needs to orchestrate repository creation across multiple external systems (GitHub API, configuration repositories, template repositories) while maintaining clean separation between business logic and infrastructure concerns. The system must support multiple deployment models (CLI, REST API, Azure Functions, MCP server) without duplicating business logic.

Previous approaches considered:

- Monolithic architecture with mixed concerns - leads to tight coupling and difficult testing
- Separate implementations per interface - causes business logic duplication and inconsistency
- Infrastructure types in core domain - creates circular dependencies and tight coupling

## Decision

Use **Hexagonal Architecture** (Ports and Adapters pattern) to separate business logic from infrastructure implementations:

- **Hexagon (Core)**: `core` - Business logic for repository creation orchestration, visibility resolution, configuration merging
- **Ports (Interfaces)**: Traits defining contracts for external systems (GitHub operations, configuration loading, template processing)
- **Adapters (Implementations)**: `github_client`, `config_manager`, `template_engine` - Concrete implementations of port interfaces
- **Dependency Direction**: Core business logic depends only on port abstractions, never on concrete adapters or external SDKs

This enables multiple user interfaces (CLI, API, MCP) to share the same business logic while swapping out implementations for testing or different deployment scenarios.

## Consequences

**Enables:**

- Single business logic implementation shared across all interfaces (CLI, API, Azure Functions, MCP)
- Easy testing with mock implementations of ports
- Clear boundaries between domain logic and infrastructure concerns
- Adding new external integrations without modifying core logic
- Independent evolution of interfaces and implementations

**Forbids:**

- Direct imports of external SDKs (`octocrab`, `handlebars`) in `repo_roller_core`
- Business logic in adapter crates (github_client, config_manager, template_engine)
- Concrete infrastructure types in core domain (except via re-export pattern for pragmatic cases)

**Trade-offs:**

- Additional abstraction layer adds some complexity
- Need for trait objects or generic parameters in some cases
- Pragmatic exceptions (re-exports) required for some infrastructure-defined types to avoid circular dependencies

## Alternatives considered

### Option A: Monolithic architecture

**Why not**: Business logic mixed with GitHub API calls, template rendering, and HTTP handling makes testing difficult. Changes to external APIs require touching business logic. Cannot reuse logic across different interfaces.

### Option B: Separate CLI and API implementations

**Why not**: Duplicates business logic between CLI and API codebases. Inconsistent behavior between interfaces. Double maintenance burden for bug fixes and features.

### Option C: Layered architecture without ports

**Why not**: Direct dependencies on concrete implementations make testing difficult. Cannot swap implementations without modifying core. Tight coupling to specific libraries (octocrab, handlebars).

## Implementation notes

**Key boundaries:**

- `repo_roller_core` never imports `octocrab`, `handlebars`, or other external SDKs directly
- Port traits live in `repo_roller_core` (e.g., `GitHubOperations`, `ConfigurationProvider`, `TemplateProcessor`)
- Adapter implementations live in separate crates (`github_client`, `config_manager`, `template_engine`)
- User interfaces (`repo_roller_cli`, `repo_roller_api`, `repo_roller_mcp`) depend on both core and adapters

**Pragmatic exception - Type re-exports:**
Some infrastructure-defined types (like `RepositoryVisibility` from `config_manager`) are re-exported through `repo_roller_core` to avoid circular dependencies. See [constraints.md](../spec/constraints.md#pragmatic-exception-for-infrastructure-defined-types) for detailed rationale.

**Testing strategy:**

- Unit tests in core use mock implementations of port traits
- Integration tests in `integration_tests` crate test real adapter implementations
- Contract tests verify adapters correctly implement port interfaces

**Adding new external integrations:**

1. Define port trait in `repo_roller_core` (e.g., `pub trait SlackNotifier`)
2. Create adapter crate with implementation (e.g., `slack_client`)
3. Core orchestration uses trait, never knows about concrete implementation
4. Wire up concrete adapter in user interface crates

## Examples

**Business logic in core (adapter-agnostic):**

```rust
// repo_roller_core - never knows about octocrab or handlebars
pub struct RepositoryCreationOrchestrator<G, T, C>
where
    G: GitHubOperations,
    T: TemplateProcessor,
    C: ConfigurationProvider,
{
    github: Arc<G>,
    templates: Arc<T>,
    config: Arc<C>,
}

impl<G, T, C> RepositoryCreationOrchestrator<G, T, C>
where
    G: GitHubOperations,
    T: TemplateProcessor,
    C: ConfigurationProvider,
{
    pub async fn create_repository(
        &self,
        request: CreateRepositoryRequest,
    ) -> RepoRollerResult<Repository> {
        // Pure business logic - no infrastructure concerns
        let config = self.config.resolve_configuration(&request.organization).await?;
        let content = self.templates.process_template(&request.template, &config).await?;
        let repo = self.github.create_repository(&request.name, content).await?;
        Ok(repo)
    }
}
```

**Adapter implementation:**

```rust
// github_client crate - implements port interface
pub struct GitHubClient {
    client: Octocrab,
}

#[async_trait]
impl GitHubOperations for GitHubClient {
    async fn create_repository(
        &self,
        name: &RepositoryName,
        content: RepositoryContent,
    ) -> Result<Repository, GitHubError> {
        // Octocrab-specific implementation
        let repo = self.client
            .repos(&name.owner(), &name.name())
            .create()
            .await?;
        Ok(repo)
    }
}
```

**Wiring in CLI:**

```rust
// repo_roller_cli - wires up concrete implementations
let github_client = GitHubClient::new(token);
let template_engine = HandlebarsTemplateEngine::new();
let config_manager = FileBasedConfigManager::new();

let orchestrator = RepositoryCreationOrchestrator::new(
    Arc::new(github_client),
    Arc::new(template_engine),
    Arc::new(config_manager),
);

// Business logic is the same for CLI, API, MCP
orchestrator.create_repository(request).await?;
```

## References

- [System Architecture Overview](../spec/architecture/system-overview.md)
- [Component Responsibilities](../spec/responsibilities.md)
- [Implementation Constraints](../spec/constraints.md)
- Hexagonal Architecture: <https://alistair.cockburn.us/hexagonal-architecture/>
- Ports and Adapters: <https://herbertograca.com/2017/09/14/ports-adapters-architecture/>
