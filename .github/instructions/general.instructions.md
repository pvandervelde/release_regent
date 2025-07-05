# Copilot Instructions (Repository) - General

## general

**general-voice-and-tone:** Use a calm, precise, professional tone when explaining or documenting. Avoid overly casual
phrasing. Keep comments and responses focused, technical, and respectful.

**general-focus:** Focus on the task at hand. Avoid distractions and stay on topic.
If you need to switch tasks, make sure to finish the current task first.


## scm

**scm-hygiene:** Commit changes frequently and in small increments. Follow the `scm-commit-message` format for commit messages. Use
`git fetch --prune` and `git pull` to update your local branch before pushing changes.

**scm-git-pull-request-title:** The pull request title should follow the conventional commit format.
`<type>(<scope>): <subject>` where `type` is one of the following: `feat`, `fix`, `chore`, `docs`,
`style`, `refactor`, `perf`, `test`.

**scm-branch-naming:** The branch name should be a brief summary of the changes being made. Branch
names should be in lowercase and use hyphens to separate words. For example, `fix-bug-in-login-page`
or `feature-add-new-user`.

**scm-commit-message:** For commit messages the
type should be one of the following: `feat`, `fix`, `chore`, `docs`,
`style`, `refactor`, `perf`, `test`. The scope should be the name of the module or component being changed. The subject should
be a short description of the change. The `work_item_ref` is one of the following issue references:
`references` or `related to` followed by the issue number.
Finally those parts make the following format for commit messages:

```text
type(scope): subject

description

 references <work_item_ref>
```


## workflow-guidelines

**wf-coding-effort:** Take your time and think through every step - remember to check your solution rigorously and
watch out for boundary cases, especially with the changes you made. Your solution must be perfect.
If not, continue working on it. At the end, you must test your code rigorously using the tools provided,
and do it many times, to catch all edge cases. If it is not robust, iterate more and make it perfect.
Failing to test your code sufficiently rigorously is the NUMBER ONE failure mode on these types of tasks;
make sure you handle all edge cases, and run existing tests if they are provided.

**wf-code-style:** All code should be easy to understand and maintain. Use clear and descriptive
names for variables, functions, and classes. Always follow the coding standards and best practices
for the programming language being used.

**wf-unit-test-coverage:** All business logic should be covered by unit tests. We're aiming to cover
all input and output paths of the code. This includes edge cases and error handling. Use coverage
tools to measure the test coverage and use mutation testing to ensure that the tests are
effective.

**wf-unit-test-check:** When you think you've solved the presented problem, run all available tests. Fix any issues that
you find.

**wf-unit-test-create-new:** Whenever you create a new test, run it to verify that it passes. If it doesn't pass, revise
either the test (or the code it tests) until the test passes.

**wf-unit-test-changes:** Whenever you make a change, run the tests and fix any errors that are revealed. Fix one error at
a time and provide an explanation of why you think the change you made fixes the error

**wf-test-methods:** Employ different test approaches to get good coverage of both happy path
and error handling. Consider approaches like unit tests, property based testing, fuzz testing,
integration tests, end-to-end tests, and performance tests. Use the appropriate testing
frameworks and tools for the programming language being used.

**wf-documentation:** The coding task is not complete without documentation. All code should be
well-documented. Use comments to explain the purpose of complex code and to provide context for
future developers. Use docstrings to document functions, classes, and modules. The documentation
should be clear and concise.

**wf-documentation-standards:** Follow the documentation standards and best practices for the
programming language being used.


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


