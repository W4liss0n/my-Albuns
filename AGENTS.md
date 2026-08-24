# Agent guidance

## Language

Product documentation is written in Portuguese; process scaffolding is written in English. The split follows the audience, not the file type.

**Portuguese** — everything a person reads to understand the product:

- the canonical specification, `CONTEXT.md` and `README.md`;
- ADRs and design documents under `docs/adr/` and `docs/design/`;
- research under `docs/research/`;
- ticket titles and acceptance criteria.

**English** — everything that exists to be parsed or acted on by a tool or an agent:

- this file and the documents under `docs/agents/`;
- GitHub issue structural fields, states, and labels;
- structural ticket labels: `What to build:`, `Blocked by:`, `Type:`, `Status:`, `Normative sources:`;
- frontmatter keys (`status`, `document`, `date`, `updated`, `ticket`, `platform`, `implementation-readiness`);
- frontmatter, triage, ticket-type and wayfinding values (`accepted`, `proposed`, `superseded`, `historical`, `current`, `ready-for-agent`, `ready-for-human`, `needs-triage`, `needs-info`, `wontfix`, `spike`, `decision`, `design`, `implementation`, `research`, `prototype`, `grilling`, `task`, `claimed`, `resolved`).

A ticket therefore mixes both: English labels and states around a Portuguese title and Portuguese criteria. That is intentional — the labels are an interface, the criteria are prose.

Do not translate an identifier that another document or skill matches on. When adding a new field or state, keep it English and add it to the lists above.

## Agent skills

### Workflow routing

When the appropriate skill or flow is unclear, use `$ask-matt` before proceeding.

### Code review

When reviewing a diff or deciding whether work is ready to integrate, read
`CODING_STANDARDS.md`.

### UI reference

Before implementing or comparing application visuals, read
`docs/references/ui-programa-diagramacao/README.md`; it identifies the only
current visual reference and the precedence of later accepted decisions.

### Issue tracker

Issues are tracked as GitHub issues in `W4liss0n/my-Albuns` via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

The repository uses the canonical Matt Pocock triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

The repository uses a single domain context at `CONTEXT.md`, with architectural decisions under `docs/adr/`. See `docs/agents/domain.md`.
