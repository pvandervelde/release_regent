# Contributing to Release Regent

Thank you for your interest in contributing to Release Regent! This document
provides guidelines and information for contributors.

## Code of Conduct

This project adheres to a code of conduct that fosters an open and welcoming
environment. Please read and follow our Code of Conduct.

## Getting Started

### Prerequisites

- Rust 1.70.0 or later
- Git
- A GitHub account

### Setting Up the Development Environment

1. **Fork and clone the repository**

   ```bash
   git clone https://github.com/YOUR_USERNAME/release_regent.git
   cd release_regent
   ```

2. **Install Rust dependencies**

   ```bash
   cargo build
   ```

3. **Run the test suite**

   ```bash
   cargo test --workspace
   ```

4. **Verify code formatting and linting**

   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   ```

## Development Workflow

### Branch Naming

Use descriptive branch names following this pattern:

- `feat/description` - for new features
- `fix/description` - for bug fixes
- `chore/description` - for maintenance tasks
- `docs/description` - for documentation changes

### Commit Messages

Follow the Conventional Commits specification:

```text
type(scope): subject

description

references #issue_number
```

**Types**: `feat`, `fix`, `chore`, `docs`, `style`, `refactor`, `perf`, `test`

**Example**:

```text
feat(core): add semantic version calculation

Implement semantic version bump logic based on conventional commit analysis.
Support major, minor, and patch version increments.

references #15
```

### Pull Request Process

1. **Create a feature branch** from `main`
2. **Make your changes** following the coding standards
3. **Add or update tests** for your changes
4. **Ensure all tests pass** locally
5. **Update documentation** if needed
6. **Submit a pull request** with a clear description

### Pull Request Template

Please use this template for your pull requests:

```markdown
## Description
Brief description of the changes

## Type of Change
- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update

## Testing
- [ ] Unit tests pass
- [ ] Integration tests pass (if applicable)
- [ ] Manual testing completed

## Checklist
- [ ] Code follows the project's coding standards
- [ ] Self-review of the code completed
- [ ] Code is commented, particularly in hard-to-understand areas
- [ ] Corresponding changes to documentation made
- [ ] Tests added that prove the fix is effective or the feature works
- [ ] New and existing unit tests pass locally

## References
References #issue_number
```

## Coding Standards

### Rust Guidelines

- **Follow the Rust Style Guide**: Use `cargo fmt` for formatting
- **Use cargo check tools**: Address all linting warnings with `cargo clippy`
- **Error Handling**: Use `Result` types and the `?` operator appropriately
- **Documentation**: Add doc comments for public APIs
- **Testing**: Write unit tests for all business logic

### Module Organization

The project uses a workspace structure:

```text
crates/
├── core/          # Business logic and core functionality
├── github_client/ # GitHub API interactions
├── cli/           # Command-line interface
└── az_func/       # Azure Function runtime
```

### Error Handling

- Use `thiserror` for custom error types
- Provide meaningful error messages
- Include context information in errors
- Use appropriate error categories

### Testing

- Write unit tests for all public functions
- Use descriptive test names: `test_function_condition_expected_result`
- Include edge cases and error scenarios
- Aim for high test coverage

## Code Review Process

1. **Automated Checks**: All PRs must pass CI checks
2. **Code Review**: At least one maintainer review required
3. **Testing**: Verify tests cover new functionality
4. **Documentation**: Ensure changes are documented

## Reporting Issues

### Bug Reports

Include:

- Steps to reproduce
- Expected vs actual behavior
- Environment details (OS, Rust version)
- Relevant logs or error messages

### Feature Requests

Include:

- Problem description
- Proposed solution
- Use case examples
- Alternative solutions considered

## Security

Report security vulnerabilities privately to the maintainers. Do not create
public issues for security concerns.

## License

By contributing, you agree that your contributions will be licensed under the
same license as the project.

## Getting Help

- **Issues**: Create a GitHub issue for bugs or feature requests
- **Discussions**: Use GitHub Discussions for questions
- **Documentation**: Check the project README and specification

## Recognition

Contributors will be recognized in the project's CONTRIBUTORS.md file.

Thank you for contributing to Release Regent!
