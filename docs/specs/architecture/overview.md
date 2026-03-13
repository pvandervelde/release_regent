# System Architecture Overview

**Last Updated**: 2026-03-13
**Status**: Updated — see ADR-002

## High-Level Architecture

Release Regent is a containerised, event-driven service that processes GitHub webhooks to
automate release management workflows.

> **Note**: Earlier versions of this document described a serverless (Azure Functions / AWS
> Lambda) deployment model. That target has been superseded by a container-based deployment
> model. See [ADR-002](../../adr/ADR-002-long-running-server-deployment-model.md) for the
> rationale.

### Architecture Principles

**Event-Driven Processing**: All workflows triggered by GitHub webhook events
**Container-Based Deployment**: Runs as an OCI container in any container orchestration platform
**Idempotent Operations**: All operations safe to retry without side effects
**Single Responsibility**: Each component has a focused, well-defined purpose

### System Context

```mermaid
C4Context
    title System Context Diagram for Release Regent

    Person(maintainer, "Repository Maintainer", "Manages releases for repositories")
    Person(developer, "Developer", "Contributes code changes")
    System(releaseregent, "Release Regent", "Automates release management")
    System_Ext(github, "GitHub", "Source code hosting and API")
    System_Ext(cloud, "Cloud Provider", "Container orchestration platform")

    Rel(developer, github, "Merges pull requests")
    Rel(github, releaseregent, "Sends webhook events")
    Rel(releaseregent, github, "Creates releases and PRs")
    Rel(maintainer, github, "Reviews release PRs")
    Rel(releaseregent, cloud, "Hosted on")
```

### Container View

```mermaid
C4Container
    title Container Diagram for Release Regent

    Container(function, "Server", "Docker / OCI container", "Long-running Axum HTTP server")
    Container(core, "Core Engine", "Rust", "Business logic and workflow orchestration")
    Container(github_client, "GitHub Client", "Rust", "GitHub API integration")
    Container(config, "Configuration", "YAML", "Application and repository settings")

    Container_Ext(github_api, "GitHub API", "REST API", "Repository operations")
    Container_Ext(github_webhooks, "GitHub Webhooks", "HTTP", "Event notifications")
    ContainerDb_Ext(secrets, "Secret Store", "Key Vault/Secrets Manager", "Credentials storage")

    Rel(github_webhooks, function, "POST webhook events", "HTTPS")
    Rel(function, core, "Invokes", "Function call")
    Rel(core, github_client, "Uses", "Function call")
    Rel(github_client, github_api, "API calls", "HTTPS")
    Rel(core, config, "Reads", "File system")
    Rel(github_client, secrets, "Retrieves tokens", "HTTPS")
```

## Component Architecture

### Processing Flow

```mermaid
flowchart TD
    A[GitHub Webhook] --> B[Function Host]
    B --> C[Webhook Processor]
    C --> D{Event Type?}

    D -->|Merged Regular PR| E[Release Orchestrator]
    D -->|Merged Release PR| F[Release Automator]
    D -->|Other| G[Ignore]

    E --> H[Version Calculator]
    E --> I[PR Manager]
    E --> J[Branch Manager]

    F --> K[Release Manager]
    F --> L[Tag Manager]

    I --> M[GitHub API Client]
    J --> M
    K --> M
    L --> M

    H --> N[Git Conventional Parser]

    style C fill:#e1f5fe
    style E fill:#f3e5f5
    style F fill:#f3e5f5
    style M fill:#fff3e0
```

### Core Components

#### 1. Server (Container Host)

**Purpose**: Long-running HTTP server hosting the webhook intake
**Technology**: Axum HTTP server (`crates/server`), deployed as an OCI container
**Responsibilities**:

- Receive and signature-validate incoming webhooks (via `github-bot-sdk`)
- Route validated events into the core processing pipeline
- Handle environment configuration and startup
- Expose health check endpoint for container orchestration probes

#### 2. Webhook Processor

**Purpose**: Event validation and routing
**Location**: `crates/core/src/webhook_processor.rs`
**Responsibilities**:

- Validate webhook signatures
- Parse and validate event payloads
- Route events to appropriate handlers
- Generate correlation IDs for tracing

#### 3. Release Orchestrator

**Purpose**: Coordinate release PR workflow
**Location**: `crates/core/src/release_orchestrator.rs`
**Responsibilities**:

- Process merged regular PRs
- Calculate semantic versions
- Orchestrate PR creation and updates
- Handle error recovery and logging

#### 4. Release Automator

**Purpose**: Create GitHub releases
**Location**: `crates/core/src/release_automator.rs`
**Responsibilities**:

- Process merged release PRs
- Extract version from PR information
- Create Git tags and GitHub releases
- Clean up release branches

#### 5. GitHub API Client

**Purpose**: All GitHub interactions
**Location**: `crates/github_client/src/`
**Responsibilities**:

- Authenticate with GitHub API
- Execute repository operations
- Handle rate limiting and retries
- Manage installation tokens

## Data Flow Architecture

### Webhook Processing Pipeline

```mermaid
sequenceDiagram
    participant G as GitHub
    participant F as Server (Container)
    participant W as Webhook Processor
    participant R as Release Orchestrator
    participant P as PR Manager
    participant A as GitHub API

    G->>F: POST /webhook (PR merged)
    F->>W: Process event
    W->>W: Validate signature
    W->>W: Parse payload
    W->>R: Route to orchestrator
    R->>R: Load configuration
    R->>R: Calculate version
    R->>P: Find/create release PR
    P->>A: Search existing PRs
    A-->>P: PR search results
    P->>A: Create/update PR
    A-->>P: PR operation result
    P-->>R: Operation complete
    R-->>W: Processing complete
    W-->>F: Success response
    F-->>G: HTTP 200 OK
```

### Release Creation Pipeline

```mermaid
sequenceDiagram
    participant G as GitHub
    participant F as Server (Container)
    participant W as Webhook Processor
    participant A as Release Automator
    participant R as Release Manager
    participant API as GitHub API

    G->>F: POST /webhook (Release PR merged)
    F->>W: Process event
    W->>A: Route to automator
    A->>A: Extract version from PR
    A->>A: Generate release notes
    A->>R: Create release
    R->>API: Create Git tag
    API-->>R: Tag created
    R->>API: Create GitHub release
    API-->>R: Release created
    R->>API: Delete release branch
    API-->>R: Branch deleted
    R-->>A: Release complete
    A-->>W: Processing complete
    W-->>F: Success response
    F-->>G: HTTP 200 OK
```

## Integration Architecture

### External System Integrations

#### GitHub API Integration

**Authentication**: GitHub App with JWT and installation tokens
**Rate Limiting**: 5,000 requests per hour per installation
**Retry Strategy**: Exponential backoff with circuit breaker
**API Versions**: REST API v3 with GraphQL v4 for future enhancements

#### Secret Management Integration

**Azure**: Azure Key Vault with Managed Identity
**AWS**: AWS Secrets Manager with IAM roles
**Access Pattern**: On-demand retrieval with in-memory caching
**Rotation**: Automated rotation with zero-downtime updates

#### Configuration Management

**Storage**: YAML files in repository or centralized configuration
**Loading**: Hierarchical loading (app defaults → repo overrides)
**Validation**: Schema-based validation with clear error messages
**Hot Reload**: Configuration changes applied without restart

### Internal Component Integration

#### Service Communication

**Pattern**: Direct function calls within same process
**Error Handling**: Result types with explicit error propagation
**Tracing**: Correlation ID propagation across all components
**Testing**: Dependency injection for unit test isolation

#### Data Sharing

**Configuration**: Shared configuration context across components
**State**: Stateless design with all context passed explicitly
**Caching**: In-memory caching for GitHub tokens and repository metadata
**Persistence**: No persistent storage required

## Deployment Architecture

### Container Deployment

Release Regent runs as an OCI container image built from `crates/server`. It can be hosted on
any container orchestration platform (Kubernetes, Azure Container Apps, AWS ECS, Docker Compose).
See [ADR-002](../../adr/ADR-002-long-running-server-deployment-model.md) for the full rationale.

```mermaid
graph TB
    subgraph "Container Platform (Kubernetes / ACA / ECS)"
        subgraph "Compute"
            C[rr-server container]
            RS[Replica set / horizontal pod autoscaler]
        end

        subgraph "Security"
            KV[Secret Store\nKey Vault / Secrets Manager]
            IAM[Workload Identity / IAM role]
        end

        subgraph "Observability"
            L[Structured logs\nstdout JSON]
            M[Metrics scrape endpoint]
            A[Alerts]
        end

        subgraph "Networking"
            ING[Ingress / Load Balancer]
            DNS[Custom Domain + TLS]
        end
    end

    C --> KV
    C --> L
    C --> M
    RS --> C
    ING --> C
    DNS --> ING
    IAM --> C
    IAM --> KV
    M --> A
```

### Infrastructure Components

#### Compute Resources

**Container image**: Built with a multi-stage Dockerfile; final image is a minimal
`debian:bookworm-slim` or `distroless` image containing only the `rr-server` binary.

**Scaling**: Horizontal pod / task scaling driven by CPU or request-rate metrics.
A minimum of one replica is required; the application is stateless so any number of
replicas can run behind a load balancer.

**Health probes**:

- Liveness: `GET /` → HTTP 200
- Readiness: `GET /` → HTTP 200

#### Storage Resources

**Configuration Storage**: Git repositories or cloud configuration services
**Temporary Storage**: In-memory processing only (bounded `mpsc` channel)
**Log Storage**: Container stdout captured by the platform's native log aggregator

#### Network Resources

**Ingress / Load Balancer**: Routes HTTPS traffic from GitHub to the container; terminates TLS
**Private Networking**: Restrict egress to GitHub API endpoints; no inbound except webhook port
**Load Balancing**: Platform ingress controller (nginx, ALB, Azure Application Gateway)

## Security Architecture

### Defense in Depth

```mermaid
graph TB
    subgraph "Network Security"
        WAF[Web Application Firewall]
        TLS[TLS 1.3 Encryption]
        DDOS[DDoS Protection]
    end

    subgraph "Application Security"
        SIG[Webhook Signature Validation]
        AUTH[GitHub App Authentication]
        RBAC[Role-Based Access Control]
    end

    subgraph "Data Security"
        ENCRYPT[Data Encryption]
        MASK[Sensitive Data Masking]
        AUDIT[Audit Logging]
    end

    WAF --> SIG
    TLS --> AUTH
    DDOS --> RBAC
    SIG --> ENCRYPT
    AUTH --> MASK
    RBAC --> AUDIT
```

### Security Layers

#### Network Layer

- TLS 1.3 for all external communications
- Web Application Firewall for attack protection
- DDoS protection at cloud provider level
- Private networking for internal communications

#### Application Layer

- Webhook signature validation using HMAC-SHA256
- GitHub App authentication with short-lived tokens
- Input validation and sanitization
- Output encoding to prevent injection attacks

#### Data Layer

- Encryption in transit and at rest
- Sensitive data masking in logs
- Audit trail for all operations
- Data minimization principles

## Observability Architecture

### Monitoring Stack

```mermaid
graph LR
    subgraph "Application"
        APP[Release Regent]
        LOGS[Structured Logs]
        METRICS[Custom Metrics]
        TRACES[Distributed Traces]
    end

    subgraph "Collection"
        AGENT[Telemetry Agent]
        BUFFER[Buffer/Queue]
    end

    subgraph "Storage"
        LOGSTORE[Log Storage]
        METRICSTORE[Metrics Database]
        TRACESTORE[Trace Storage]
    end

    subgraph "Analysis"
        DASHBOARD[Dashboards]
        ALERTS[Alerting]
        ANALYTICS[Log Analytics]
    end

    APP --> LOGS
    APP --> METRICS
    APP --> TRACES
    LOGS --> AGENT
    METRICS --> AGENT
    TRACES --> AGENT
    AGENT --> BUFFER
    BUFFER --> LOGSTORE
    BUFFER --> METRICSTORE
    BUFFER --> TRACESTORE
    LOGSTORE --> DASHBOARD
    METRICSTORE --> DASHBOARD
    TRACESTORE --> DASHBOARD
    METRICSTORE --> ALERTS
    LOGSTORE --> ANALYTICS
```

### Telemetry Strategy

#### Structured Logging

- JSON format with consistent schema
- Correlation ID tracking across components
- Context-rich error information
- Performance timing for operations

#### Metrics Collection

- Business metrics (success rates, processing times)
- System metrics (memory, CPU, network)
- GitHub API metrics (rate limits, response times)
- Custom application metrics

#### Distributed Tracing

- Request lifecycle tracking
- Component interaction visualization
- Performance bottleneck identification
- Error context preservation

## Scalability Architecture

### Horizontal Scaling

```mermaid
graph TB
    subgraph "Load Distribution"
        LB[Load Balancer]
        R1[Region 1]
        R2[Region 2]
        R3[Region 3]
    end

    subgraph "Auto Scaling"
        AS[Auto Scaler]
        M[Metrics]
        Q[Queue Depth]
    end

    subgraph "Resource Management"
        CPU[CPU Limits]
        MEM[Memory Limits]
        CONN[Connection Pooling]
    end

    LB --> R1
    LB --> R2
    LB --> R3
    M --> AS
    Q --> AS
    AS --> CPU
    AS --> MEM
    AS --> CONN
```

### Scaling Strategies

#### Function Scaling

- Auto-scale based on queue depth and processing time
- Regional deployment for global availability
- Reserved capacity for critical operations
- Burst handling with overflow protection

#### Resource Optimization

- Memory and CPU allocation based on workload patterns
- Connection pooling for GitHub API calls
- Caching strategies for frequently accessed data
- Lazy loading of configuration and dependencies

#### Performance Optimization

- Asynchronous processing throughout
- Batch operations where possible
- Smart retry strategies with circuit breakers
- Efficient data structures and algorithms

## Future Architecture Considerations

### Extensibility Points

#### Plugin Architecture

- External versioning strategy plugins
- Custom notification handlers
- Template engine extensions
- Workflow customization hooks

#### Multi-Tenancy

- Repository isolation and security
- Per-tenant configuration and limits
- Billing and usage tracking
- Resource quota management

#### Advanced Features

- Multi-repository release coordination
- Advanced approval workflows
- Integration with external CI/CD systems
- Analytics and reporting capabilities
