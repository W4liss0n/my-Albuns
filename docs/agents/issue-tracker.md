# Issue tracker: Local Markdown

Issues for this repository live as Markdown files under `.scratch/`.

## Conventions

- One feature per directory: `.scratch/<feature-slug>/`.
- The tracker entry for the product specification is `.scratch/<feature-slug>/spec.md`.
- The canonical product specification may remain under `docs/specs/`; the tracker entry must link to it explicitly.
- Implementation issues use one file per ticket at `.scratch/<feature-slug>/issues/<NN>-<slug>.md`, numbered from `01`.
- Never combine every ticket into one file.
- Triage state is recorded as a `Status:` line near the top of each ticket.
- Work kind may be recorded as an optional `Type:` line using `spike`, `decision`, `design` or `implementation`.
- Blocking edges are recorded as a `Blocked by:` line.
- Each ticket declares its `Normative sources:`. Criteria remain self-contained, but may only detail — never redefine — those sources.
- Comments and work history are appended under `## Comments`.

## Publishing

When a skill says to publish a spec or issue, create the corresponding Markdown file under `.scratch/<feature-slug>/`.

## Working the frontier

A ticket is available when every ticket listed in `Blocked by:` is complete. Work available tickets in number order unless a ticket documents another priority.

`ready-for-agent` means the ticket is fully specified for its declared work type. A decision that still requires a product choice uses `ready-for-human`; do not redefine tracker states inside a feature specification.
