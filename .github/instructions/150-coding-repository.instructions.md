---
applyTo: "**"
---

# Copilot Instructions (Repository)

## coding

**coding-design-architecture:** Design modular, maintainable system components using appropriate technologies and frameworks. Ensure that integration
points are clearly defined and documented.

**coding-whitespace:** Always leave a whitespace between a line of code and a comment. This improves readability and helps to distinguish
between code and comments.

**coding-style:** Follow the style guides for the language. Use the appropriate formatters to format your code. This will
help ensure that the code is consistent and easy to read.

**coding-comments:** Use comments to explain the purpose and reasoning behind non-obvious code. Focus on *why* the code is written
this way — for example, domain-specific constraints, algorithmic trade-offs, or error handling strategy.
Avoid commenting obvious control flow or syntax.

**coding-tests-always:** After implementing any logic, immediately write unit tests that verify its correctness.
Include realistic inputs, edge cases, and error conditions. Use the naming convention rules for test files.

**coding-test-granularity:** Each test should verify one behavior or input class. Use descriptive names like `test_parse_empty_string_fails`.
Prefer many small, specific tests over a few broad ones.

**coding-test-execution:** Always assume tests should be executed. If tests cannot be run, clearly state this and provide expected output or
status. Use `cargo test` as the default unless a better method is available.

**coding-review-before-commit:** Before committing code, review it for correctness, style, and test coverage. Ensure that **all** rules are followed,
that the code is as simple as it could be, and that the code is ready for production use. Now is the time to refactor
or simplify the code if needed.


