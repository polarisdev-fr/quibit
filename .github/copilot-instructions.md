## Git workflow

- Never commit or push directly to `main`. Create a task branch from an up-to-date `main`, using a prefix such as `feat/`, `fix/`, or `ci/`.
- Commit small logical changes with conventional commit messages (`feat:`, `fix:`, `refactor:`, `ci:`, `docs:`, or `chore:`), then push the branch to `origin` after local checks pass.
- Never force push or rewrite history that has already been pushed.
- Never merge branches yourself. Merge through a GitHub pull request only after CI checks pass.
