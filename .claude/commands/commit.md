---
description: Crée un commit Git conventionnel pour un seul changement logique
allowed-tools: Bash(git status:*), Bash(git diff:*), Bash(git add:*), Bash(git commit:*), Bash(git log:*)
---

Create a Git commit for the current changes.

Requirements:
- Analyze all staged changes.
- If nothing is staged, stage only the files related to the current task.
- Review the diff before committing.
- Ensure the commit contains only one logical change. If unrelated changes are detected, stop and explain why instead of creating a commit.
- Generate a concise commit message in English using the imperative mood.
- Follow Conventional Commits:
  - `feat:`
  - `fix:`
  - `refactor:`
  - `perf:`
  - `docs:`
  - `test:`
  - `build:`
  - `ci:`
  - `chore:`
- Do not add co-authors.
- Do not create merge commits.
- Do not amend existing commits unless explicitly requested.
- After generating the message, execute the commit.
