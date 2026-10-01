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

## Local folders

- `.tools/` holds only the local toolchain and tool reports: `cargo/`, `rustup/`,
  `rustup-init.exe`, `bootstrap/`, `edge-driver/`, `webview2-driver/`,
  `tauri-driver/`, `validation/` and `native-gate-build.json`. Never store
  investigation evidence there.
- `.scratch/` holds local work that stays out of Git. `.gitignore` ignores only
  the `.scratch/` folders that versioned code or scripts use; any other untracked
  file there stays visible on purpose, because native gates treat it as dirty
  source (`scripts/Gate-SourceProvenance.ps1`, checked by
  `scripts/Test-ProjectCloseGate.mjs`). Do not ignore `.scratch/` as a whole.
- The frozen legacy trackers `.scratch/programa-diagramacao/` and
  `.scratch/esqueleto-ponta-a-ponta/` are versioned because GitHub issues link to
  them: do not edit, move or delete them.
- `.scratch/fixtures/` holds real photos read by development QA modes, UI
  acceptance scenarios and ignored tests. Do not delete it during cleanup.
- `.scratch/planos/` holds plans. The repository is public and plans may name
  client folders, so plans stay local.
- Create one `.scratch/<YYYY-MM-DD>-<topic>/` folder per new investigation and
  exclude it in `.git/info/exclude`. It becomes disposable once a research
  document records the result.
- Scripts write gate evidence under `.scratch/` (`ui-acceptance/`, `*-evidence/`,
  `*-gate/`); those outputs are disposable.
- Tracked files and GitHub issues must not depend on `.scratch/` content. Record
  durable decisions and measurements under `docs/`; citing local evidence as
  outside Git is allowed.
- Do not rename or move files under `docs/`: GitHub issues link to them by path.

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

Also read `.interface-design/system.md` before UI implementation or review. It
records the author's accepted visual direction and reusable interaction patterns;
apply them within their documented scope and keep them current when later explicit
user decisions refine them.

### User-facing copy

Before adding or changing user-facing text, read
`docs/research/2026-09-18-padroes-de-escrita-para-interfaces.md` and the accepted
writing guide in `docs/design/0043-plano-de-simplificacao-dos-textos.md`.
Apply the guide to new features as well as existing flows, including tooltips,
accessible names, confirmations, and Rust errors forwarded to the UI. Verify
the actual outcome and available next action before choosing the wording;
keep support diagnostics in the existing diagnostic channel.

### Issue tracker

Issues are tracked as GitHub issues in `W4liss0n/my-Albuns` via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

The repository uses the canonical Matt Pocock triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

The repository uses a single domain context at `CONTEXT.md`, with architectural decisions under `docs/adr/`. See `docs/agents/domain.md`. `docs/README.md` maps the documentation folders and the documents for each topic.

### Review findings

Before reporting audit or code-review findings, apply the admission and disposition rules in `docs/agents/review-findings.md`. Prove production reachability or concrete architectural leverage, and reconcile prior `wontfix` and `.out-of-scope/` decisions.
