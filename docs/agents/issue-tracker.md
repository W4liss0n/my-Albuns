# Issue tracker: GitHub

Issues and specs for this repository live as GitHub issues. Use the `gh` CLI for all operations.

## Repository

- The repository is `W4liss0n/my-Albuns`.
- When run inside this clone, `gh` infers the repository from `git remote -v`.
- Use `-R W4liss0n/my-Albuns` only when running outside the clone or when the repository would otherwise be ambiguous.
- Existing files under `.scratch/` are legacy local artifacts after this switch. Do not create new tracker entries there unless the tracker configuration is changed again.

## Conventions

- **Create an issue**: `gh issue create --title "..." --body "..."`. For a multi-line body, use `--body-file -`.
- **Read an issue**: `gh issue view <number> --json number,title,body,labels,comments,assignees,blockedBy,blocking,parent,subIssues`.
- **List issues**: `gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'`, with appropriate `--label`, `--state` and `--search` filters.
- **Comment on an issue**: `gh issue comment <number> --body "..."`.
- **Apply or remove labels**: `gh issue edit <number> --add-label "..."` or `--remove-label "..."`.
- **Close an issue**: `gh issue close <number> --comment "..."`.

## Ticket structure

- Product-facing titles and acceptance criteria are written in Portuguese.
- Structural fields and state identifiers remain in English, as defined by `AGENTS.md`.
- Each ticket declares its `Normative sources:`. Acceptance criteria remain self-contained, but may only detail — never redefine — those sources.
- Use GitHub comments for discussion and work history.
- Record triage state with the labels mapped in `docs/agents/triage-labels.md`.
- Use native GitHub issue dependencies for blocking relationships. A `Blocked by:` line is only a fallback when native dependencies are unavailable.
- `ready-for-agent` means the ticket is fully specified for its declared work type. A decision that still requires a product choice uses `ready-for-human`.

## Pull requests as a triage surface

**PRs as a request surface: no.** _(Set to `yes` if this repository treats external PRs as feature requests; `/triage` reads this flag.)_

When set to `yes`, PRs run through the same labels and states as issues, using the `gh pr` equivalents:

- **Read a PR**: `gh pr view <number> --comments` and `gh pr diff <number>`.
- **List external PRs for triage**: `gh pr list --state open --json number,title,body,labels,author,authorAssociation,comments`, then keep only `authorAssociation` values `CONTRIBUTOR`, `FIRST_TIME_CONTRIBUTOR` or `NONE`.
- **Comment, label or close**: use `gh pr comment`, `gh pr edit --add-label` or `--remove-label`, and `gh pr close`.

GitHub shares one number space across issues and pull requests. Resolve an ambiguous `#42` with `gh pr view 42`, then fall back to `gh issue view 42`.

## When a skill says "publish to the issue tracker"

Create a GitHub issue.

## When a skill says "fetch the relevant ticket"

Run `gh issue view <number> --json number,title,body,labels,comments,assignees,blockedBy,blocking,parent,subIssues`.

## Wayfinding operations

Wayfinder efforts use one map issue with child issues for decisions and investigations. These workflow states are separate from the triage labels used by implementation issues.

- **Map**: one issue labelled `wayfinder:map`, holding Destination, Notes, Decisions so far, Not yet specified and Out of scope.
- **Child ticket**: create it with `gh issue create --parent <map-number> --label "wayfinder:<type>"`; `<type>` is `research`, `prototype`, `grilling` or `task`. An existing issue can be attached with `gh issue edit <child> --parent <map-number>`.
- **Blocking**: create with `--blocked-by <numbers>` or add later with `gh issue edit <child> --add-blocked-by <number>`. A ticket is unblocked when every blocker is closed.
- **Frontier**: query the map's open children, then exclude issues with an open entry in `blockedBy` or an assignee. The first remaining child in map order wins.
- **Claim**: `gh issue edit <number> --add-assignee @me`; this is the session's first write.
- **Resolve**: comment with the answer, close the child, then append a one-line gist and link to the map's Decisions so far.
