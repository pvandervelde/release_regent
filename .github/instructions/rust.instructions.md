# Copilot Instructions (Repository) - Rust

## coding-rust

**rust-design-api-boundaries:** Minimize the public surface area. Expose only what is necessary using `pub(crate)` or `pub(super)`
where appropriate. Use `#[doc(hidden)]` on internals not meant for public use.

**rust-avoid-unnecessary-allocation:** Prefer `&str` or `Cow<'_, str>` over `String` when borrowing is sufficient. Avoid `Vec` when arrays
or slices suffice. Optimize for minimal allocation in performance-sensitive or embedded code.

**rust-favour-traits-over-closures:** Use traits to define behavior and interfaces. Prefer trait objects or generics over closures for
polymorphism. This improves type safety and reduces runtime overhead.

**rust-compile-first:** All Rust code suggestions must be valid and compile without errors. Prefer suggestions that are `cargo check`-safe.
If the code depends on uncertain types, traits, or lifetimes, ask clarifying questions or break the solution
into smaller, verifiable steps.

**rust-pedantic-mode:** Avoid unsafe code, unnecessary uses of `unwrap`, trait abuse, or macro-heavy logic.
Prioritize clarity, safety, and correctness over cleverness or brevity.

**rust-error-design:** Use `thiserror` to define custom error types. Return informative error messages that clearly describe the
nature of the error and, if possible, how to resolve it.

**rust-diag-checklist:** Before presenting final Rust code, run an internal diagnostic checklist:
- Are all types correct?
- Are all `Result` or `Option` paths handled?
- Are lifetimes correctly handled?
- Is ownership respected?
- Are tests present or explicitly justified?
- Have all assumptions been stated?

**rust-element-ordering:** Use the following order for elements in a module. Within each section (constants, traits, structs,
enums, functions), order items alphabetically by their name. Do not mix `impl` blocks and functions
across struct/enum boundaries; keep each type and its implementation(s) together. The order is
as follows:
- imports - organized by standard library, third-party crates, and local modules
- constants
- traits
- structs with their implementations.
- enums with their implementations.
- functions
- the main function

**rust-documentation:** For public items documentation comments are always added. For private items
documentation comments are added when the item is complex or not self-explanatory. Use `///` for
documentation comments and `//!` for module-level documentation. Add examples to the documentation
comments when possible.

**rust-modules:** When making modules in a crate create a `<module_name>.rs` file in the `src`
directory. If the module is large enough to warrant its own directory, create a directory with the
same name as the module. Place any source files for the module in the directory.

**rust-error-handling:** Use the `Result` type for functions that can return an error. Use the `?` operator
to propagate errors. Avoid using `unwrap` or `expect` unless you are certain that the value will not be
`None` or an error.

**rust-error-messages:** Use clear and descriptive error messages. Avoid using generic error messages
like "an error occurred". Instead, provide specific information about what went wrong and how to fix it.

**rust-error-types:** Use custom error types for your application. This will help you provide more
meaningful error messages and make it easier to handle errors in a consistent way. Use the `thiserror`
crate to define custom error types.

**rust-test-location:** Put unit tests in their own file. They are placed next to the file they
are testing and are named `<file_under_test>_tests.rs`. Reference them from the file under test with
an import, which is placed at the end of the other imports and usings. This pattern separates test logic from
business logic, improving clarity and minimizing rebuild times during development. This will look something like:

``` rust
#[cfg(test)]
#[path = "<file_under_test>_tests.rs"]
mod tests;
```


