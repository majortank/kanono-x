# Contributing to Kanono X

Thanks for helping improve Kanono X. Small, focused changes are easiest to review and are especially welcome around playback reliability, library scanning, accessibility, and Linux packaging.

## Before you start

1. Check existing issues and documentation.
2. Avoid committing personal music, generated databases, screenshots containing private data, or build output.
3. For larger behavior or UI changes, open an issue first so the approach can be agreed before implementation.

## Development workflow

```bash
git clone https://github.com/majortank/kanono-x.git
cd kanono-x
cargo fmt --all
cargo check --locked
```

If a change affects runtime behavior, describe how it was tested and which Linux/audio setup was used. Keep commits focused and explain user-visible changes in the pull request description.

## Pull requests

Please include:

- a short description of the problem and solution;
- screenshots or recordings for meaningful UI changes;
- reproducible steps for bug fixes;
- notes about new dependencies, external tools, or platform requirements.

By contributing, you agree that your contributions may be distributed under the project license.
