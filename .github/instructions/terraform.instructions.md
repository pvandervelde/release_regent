# Copilot Instructions (Repository) - Terraform

## coding-terraform

**tf-data-sources:** Use data sources to fetch existing resources rather than hard-coding values. Name data
sources descriptively: `data "aws_vpc" "main"`. Use locals to transform data source
outputs when needed. Avoid over-fetching data - only query what you need. Cache data
source results in locals when used multiple times.

**tf-documentation:** Add documentation comments for each resource, module, and variable. Use `#` for inline
comments, `##` for section headers. Include purpose, usage examples, and important
constraints. Use `description` attributes for all variables and outputs. Generate
README.md with terraform-docs. Document dependencies and prerequisites.

**tf-error-handling:** Use validation blocks in variables for input checking. Provide clear error messages
that explain what went wrong and how to fix it. Use precondition and postcondition
checks for complex logic. Handle optional attributes gracefully with try() function.
Use can() function for safe type checking.

**tf-file-organization:** Organize files logically: `main.tf` for primary resources, `variables.tf` for inputs,
`outputs.tf` for outputs, `versions.tf` for provider requirements. Use meaningful
file names for complex modules (`networking.tf`, `security.tf`). Keep files under
200 lines - split large files by logical grouping.

**tf-locals:** Use locals to avoid repetition and improve readability. Define common tags, computed
values, and complex expressions in locals. Group related locals logically. Use
descriptive names that explain the purpose. Prefer locals over variables for
internal calculations.

**tf-modules:** Design modules for reusability with clear interfaces. Use semantic versioning for
module releases. Minimize required variables - prefer sensible defaults. Group
related resources in modules. Use module composition over monolithic modules.
Pin module versions in production. Include examples directory with usage samples.

**tf-naming-conventions:** Use snake_case for resource names, variables, and outputs. Include environment and
purpose in resource names: `aws_instance.web_server_prod`. Use consistent prefixes
for related resources. Avoid abbreviations unless standard. Use descriptive names
that indicate the resource's purpose and scope.

**tf-outputs:** Export all values that other modules or root configs might need. Use descriptive
names and include `description` attributes. Mark sensitive outputs appropriately.
Structure complex outputs as objects rather than individual values. Document
output format and expected usage patterns.

**tf-provider-management:** Pin provider versions with `~>` operator for patch updates. Define required_providers
in terraform block. Use provider aliases for multi-region deployments. Configure
providers at the root level, not in modules. Keep provider configurations simple
and use variables for dynamic values.

**tf-resource-management:** Use consistent tagging strategy with required tags (environment, owner, project).
Use lifecycle rules to prevent accidental deletion of critical resources. Group
related resources logically. Use depends_on sparingly - prefer implicit dependencies.
Consider resource lifecycle and replacement impact when designing.

**tf-security:** Never commit secrets or sensitive data. Use variable validation for security
constraints. Mark sensitive variables and outputs appropriately. Use least
privilege IAM policies. Enable encryption at rest and in transit. Use secure
defaults and validate security configurations with policy-as-code tools.

**tf-state-management:** Use remote state storage with versioning enabled. Configure state locking to
prevent conflicts. Use separate state files for different environments. Minimize
state file blast radius by splitting large configurations. Use workspaces for
environment separation when appropriate. Never edit state files manually.

**tf-testing:** Test modules with multiple input combinations. Use terraform validate for syntax
checking. Use policy-as-code tools (OPA, Sentinel) for compliance testing.
Test infrastructure changes in non-production environments first. Use terratest
or similar for integration testing. Validate outputs match expectations.

**tf-variables:** Define explicit types for all variables. Include description and validation rules.
Provide sensible defaults where possible. Use objects for complex configurations.
Group related variables logically. Use nullable = false for required fields.
Validate inputs early with validation blocks and clear error messages.


