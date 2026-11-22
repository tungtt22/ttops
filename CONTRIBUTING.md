# Contributing to TTops

1. Read the constitution: `.specify/memory/constitution.md`. It overrides other practices.
2. New features and observable-behaviour changes follow the Spec Kit lifecycle:
   `specs/<feature>/spec.md` → `plan.md` → `tasks.md` → implementation
   (`speckit-specify`, `speckit-plan`, `speckit-tasks`, `speckit-implement`).
   Bug fixes that restore specified behaviour and pure refactors may skip `specs/`, but the PR
   must state root cause, acceptance criteria, and verification.
3. Core logic is test-first; tests use temp dirs only and never touch real user data.
4. Run `./scripts/check.sh` before opening a pull request.
   The repository is public. Never commit personal absolute paths, usernames, emails, hostnames,
   tokens, or keys — in code, tests, fixtures, or docs. Resolve locations at runtime through the
   Platform layer; use `example.com` for placeholder emails. `scripts/check-hardcode.sh` enforces
   this and runs first in `check.sh`.
5. Work on a feature branch; changes reach `main` only via a PR merged by the owner.
   No direct or force push to `main`, no `--no-verify`.
6. Tech Lead review and Tester verification run as independent sessions tracked on the Kanban
   board; verdicts are posted on the PR.
7. No AI attribution lines in commits, PRs, code, or docs.
