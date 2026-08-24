# Coding standards

Read this file during review. Product behavior remains normative in `CONTEXT.md`,
accepted ADRs, `docs/design/`, and the current visual-reference pointer in
`AGENTS.md`; this file does not restate those decisions.

## Authoritative asynchronous state

- For queued mutations, verify when revisions, dirty state, and projections are
  captured. Values used for optimistic-concurrency checks must be authoritative
  at execution time or explicitly chained from the preceding result.
- Exercise adjacent commands while a mutation is pending, in both success and
  failure paths. A local loading flag is insufficient when commands in another
  surface can observe or mutate the same session.

## Canonical ownership

- A module described as a canonical catalog owns its typed identifiers and the
  stable metadata named by the accepted design. Menus, dispatchers, and other
  consumers project from that source instead of rebuilding partial catalogs.
- Shared UI cores live in neutral modules. Their exported names, CSS classes,
  and imports must not claim ownership by one consuming flow. Context-specific
  composition remains with the owning feature.
- Apply the deletion test before accepting a new abstraction: removing a useful
  shared seam must make the same non-trivial policy reappear in multiple callers.

## Review evidence

- A static CSS or source-string check proves structure, not rendered appearance.
  Visual approval requires rendered evidence for every affected scenario in the
  repository's UI-acceptance manifest.
- If a required visual scenario cannot be captured or inspected, report it as
  unvalidated. Do not infer visual approval from typecheck, unit tests, or build
  success.
