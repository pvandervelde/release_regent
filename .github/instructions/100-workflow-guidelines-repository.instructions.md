---
applyTo: "**"
---

# Copilot Instructions (Repository)

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


