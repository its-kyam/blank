# Contributing to Blank

Thank you for your interest in contributing to Blank!

Blank aims to remain crazily simple, ridiculously fast, and amazingly small. Contributions should respect these principles.

## Before Contributing

- Check existing issues and pull requests before starting work.
- For significant changes, open an issue to discuss the proposal first.
- Keep changes focused and avoid unnecessary complexity.
- Avoid introducing dependencies without a good reason.

## Development Setup

You will need:

- Rust and Cargo
- Git

Clone the repository and enter its directory:

```bash
git clone <repository-url>
cd blank
```

## Making Changes

1. Create a branch for your changes.
2. Make your changes.
3. Add or update tests where appropriate.
4. Ensure existing functionality remains intact.
5. Submit a pull request.

## Testing

Before submitting a pull request, run:

```bash
cargo fmt --check
cargo check
cargo test
cargo clippy
```

All checks should pass before submitting.

## Pull Requests

A good pull request should:

- Have a clear title and description.
- Explain what changed and why.
- Include relevant tests.
- Avoid unrelated modifications.
- Reference related issues when applicable.

Keep pull requests focused and reasonably sized.

## Reporting Bugs

When reporting a bug, include:

- A clear description of the problem.
- Steps to reproduce it.
- Expected behaviour.
- Actual behaviour.
- Relevant error messages or logs.
- Your Rust version and operating system, when relevant.

## Feature Requests

Explain the problem you want to solve, the proposed behaviour, and why the feature belongs in Blank.

Features should align with the project's goals of simplicity, performance, and minimalism.

## Code Quality

Contributions should prioritize:

- Readability.
- Correctness.
- Performance.
- Minimal unnecessary complexity.
- Consistency with the existing codebase.

Thank you for helping improve Blank!