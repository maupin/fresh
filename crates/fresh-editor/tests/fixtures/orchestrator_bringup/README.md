# Orchestrator bring-up characterization fixtures

These JSON files reproduce the on-disk state a user accumulates by
running the orchestrator and quitting, across the three historical
persistence layouts (issue #2056).

Path values are **tokens** that the test substitutes with real
canonicalized temp dirs at runtime:

- `__PROJECT__`  — the launch cwd (`fresh .` is run here)
- `__WORKTREE__` — a separate git-worktree dir an orchestrator session runs in
- `__OTHER__`    — an unrelated project's dir

Layout each fixture is written to (see the test harness):

- v2 global  → `<data>/orchestrator/windows.json`
- v1 per-cwd → `<data>/orchestrator/<encoded-cwd>/windows.json` (migrated on first read)
- v0.3.6     → `<project>/.fresh/windows.json` (in the working tree)

The fixtures are validated by the real reader: each bring-up test
constructs an `Editor` which calls `read_persisted_windows_env` /
`Workspace::load`, so a schema mistake surfaces as a failed parse
(no sessions) rather than passing silently.
