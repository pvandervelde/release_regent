# Copilot Instructions (Repository) - Rust

## coding-rust

**rust-async-patterns:** Use `async fn` for async functions, prefer `tokio::spawn` for concurrent tasks, use `Arc<Mutex<T>>`
or channels for shared state. Avoid blocking operations in async contexts. Use `select!` for
racing futures, `.await` for sequential operations. Prefer `async-trait` for async trait methods.

**rust-attributes:** Use `#[must_use]` for functions whose return value should be used. Use `#[deprecated]` with
clear migration guidance. Use `#[allow(clippy::...)]` sparingly and with justification.
Use `#[doc = "..."]` for complex documentation that needs formatting, `///` for simple cases.

**rust-avoid-unnecessary-allocation:** Prefer `&str` or `Cow<'_, str>` over `String` when borrowing is sufficient. Avoid `Vec` when arrays
or slices suffice. Optimize for minimal allocation in performance-sensitive or embedded code.

**rust-clippy-integration:** Run clippy with `#![warn(clippy::pedantic)]` in lib.rs/main.rs. Use `#[allow(clippy::...)]`
only when necessary with a comment explaining why. Common allows: `module_name_repetitions`,
`similar_names`, `too_many_lines` when justified. Prefer fixing the code over allowing the lint.

**rust-collections:** Prefer `Vec<T>` for growable arrays, `[T; N]` for fixed-size, `&[T]` for borrowed slices.
Use `HashMap` for key-value lookups, `BTreeMap` when ordering matters, `HashSet`/`BTreeSet`
for unique collections. Consider `IndexMap` when insertion order matters with fast lookups.

**rust-compile-first:** All Rust code suggestions must be valid and compile without errors. Prefer suggestions that are `cargo check`-safe.
If the code depends on uncertain types, traits, or lifetimes, ask clarifying questions or break the solution
into smaller, verifiable steps.

**rust-dependency-management:** Group dependencies in Cargo.toml: workspace dependencies first, then essential third-party crates
(alphabetically), then optional features, then dev-dependencies. Use specific versions for critical
dependencies. Prefer workspace dependencies for multi-crate projects. Document why each dependency is needed.

**rust-design-api-boundaries:** Minimize the public surface area. Expose only what is necessary using `pub(crate)` or `pub(super)`
where appropriate. Use `#[doc(hidden)]` on internals not meant for public use.

**rust-diag-checklist:** Before presenting final Rust code, run an internal diagnostic checklist:
- Are all types correct?
- Are all `Result` or `Option` paths handled?
- Are lifetimes correctly handled?
- Is ownership respected?
- Are tests present or explicitly justified?
- Have all assumptions been stated?

**rust-documentation:** For public items documentation comments are always added. For private items documentation
comments are added when the item is complex or not self-explanatory. Use `///` for simple
documentation comments and `//!` for module-level documentation. Use `#[doc = "..."]` for
complex documentation that needs special formatting. Add examples to documentation comments
when possible, especially for public APIs.

**rust-element-ordering:** Use the following order for elements in a module. Within each section, order items alphabetically by their name,
except where noted. Keep each type and its implementation(s) together.
- Within modules, use the following order:
  1. **Module attributes** - `#![...]` that apply to the entire module
  2. **Imports** - `use` statements, organized as:
    - Standard library (`std::`, `core::`, `alloc::`) - shorter paths before longer paths
    - Third-party crates (alphabetically) - shorter paths before longer paths
    - Current crate (`crate::`) - shorter paths before longer paths
    - Local modules (relative imports like `super::`, `self::`) - shorter paths before longer paths
  3. **Module declarations** - `mod` statements (alphabetically)
  4. **Re-exports** - `pub use` statements (alphabetically)
  5. **Constants and statics** - `const` and `static` items (alphabetically)
  6. **Type aliases** - `type` definitions (alphabetically)
  7. **Macros** - `macro_rules!` definitions (alphabetically)
  8. **Traits** - trait definitions with methods in alphabetical order
  9. **Structs** - each followed immediately by all its `impl` blocks
  10. **Enums** - each followed immediately by all its `impl` blocks
  11. **Impl blocks for external types** - implementations for types not defined in this file
  12. **Free functions** - standalone functions (alphabetically)
  13. **Main function** - `fn main()` always last
- Within impl blocks:
  1. Associated constants
  2. Associated types
  3. Constructor methods (`new`, `with_*`, `from_*`, `default`, etc.)
  4. Other methods (alphabetically)
- Within trait definitions:
  1. Associated constants
  2. Associated types
  3. Required methods (alphabetically)
  4. Provided methods (alphabetically)

**rust-error-handling:** Use `Result<T, E>` for fallible operations and custom error types with `thiserror`. Use `?` for
propagation, avoid `unwrap`/`expect` except in tests or when panicking is intended. Provide clear,
actionable error messages that describe what went wrong and how to fix it. Chain errors with
`.with_context()` when using `anyhow`.

**rust-favour-traits-over-closures:** Use traits for stable interfaces, polymorphism, and when behavior needs to be implemented by external
types. Use closures for short-lived operations, functional transformations, and callbacks. Prefer
`impl Fn()` parameters over trait objects when the closure is the primary interface.

**rust-feature-flags:** Use feature flags for optional functionality. Default features should provide core functionality.
Name features descriptively (`serde-support`, `async-runtime`). Use `#[cfg(feature = "...")]`
consistently. Document feature combinations that don't work together. Avoid circular feature dependencies.

**rust-generics-conventions:** Use conventional generic parameter names: `T` for single type, `K`/`V` for key/value, `E` for error,
`F` for function/closure. Order: lifetime parameters, type parameters, const parameters. Use descriptive
names for domain-specific generics (`TStorage`, `TMessage`). Prefer `impl Trait` over generic parameters
for simple cases.

**rust-logging-conventions:** Use `tracing` for structured logging with `debug!`, `info!`, `warn!`, `error!` macros. Use spans
for request tracing. Include relevant context in log messages. Use `target` parameter for library
code. Prefer structured fields over string interpolation: `info!(user_id = %id, "User logged in")`.

**rust-methods-vs-functions:** Use methods (`&self`, `&mut self`, `self`) when the function operates on the type's data.
Use associated functions (`Self::new()`) for constructors and type-related utilities that don't
need an instance. Use free functions for utilities that work with multiple types or don't
belong to a specific type.

**rust-modules:** For simple modules, create `module_name.rs` in `src/`. For complex modules, use the newer
`src/module_name.rs` + `src/module_name/` directory structure. Keep module hierarchies shallow
when possible.

**rust-naming-conventions:** Use `snake_case` for functions, variables, modules; `PascalCase` for types, traits, enums;
`SCREAMING_SNAKE_CASE` for constants. Prefix boolean functions with `is_`, `has_`, `can_`, etc.
Use descriptive names that reveal intent. Avoid abbreviations unless they're domain-standard.

**rust-pattern-matching:** Use `if let` for single pattern matches, `match` for multiple patterns. Prefer exhaustive
matching over catch-all patterns when possible. Use `@` bindings for complex patterns.
Use guards (`if` clauses) sparingly. Order match arms from specific to general. Use `_`
for intentionally ignored values.

**rust-pedantic-mode:** Avoid `unsafe` without clear justification and documentation. Minimize `unwrap()`/`expect()` outside
of tests. Avoid overly complex trait hierarchies or excessive macro usage that reduces code clarity.
Prefer explicit over implicit when it improves readability.

**rust-performance:** Profile before optimizing. Use `cargo bench` for microbenchmarks. Prefer `Vec::with_capacity()`
when size is known. Use `&str` over `String` for temporary strings. Consider `Cow<str>` for
conditional ownership. Use `Box<[T]>` instead of `Vec<T>` for fixed-size collections. Profile
allocations with tools like `heaptrack` or `valgrind`.

**rust-string-types:** Use `&str` for string slices, `String` for owned strings, `Cow<str>` when you might need either.
Prefer `&str` in function parameters unless ownership is required. Use `format!()` sparingly in
hot paths - prefer `write!()` to a buffer. Use `Box<str>` for fixed strings that need ownership.

**rust-test-location:** Put unit tests in their own file. They are placed next to the file they
are testing and are named `<file_under_test>_tests.rs`. Reference them from the file under test with
an import, which is placed at the end of the other imports and usings. This pattern separates test logic from
business logic, improving clarity and minimizing rebuild times during development. This will look something like:

``` rust
#[cfg(test)]
#[path = "<file_under_test>_tests.rs"]
mod tests;
```


