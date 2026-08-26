# Review finding admission

Use this policy for code review, architecture review, UI audit and readiness review. Its purpose is to keep actionable defects and concrete architecture work visible while preventing unsupported hypotheses from being promoted to findings.

## Admission routes

A reportable finding must satisfy at least one route:

1. **User-reachable** — a supported sequence in the product UI reaches the behaviour. Record the sequence and the observed result. DevTools, direct component props, fake ports and test-only routes do not prove this route.
2. **Production-reachable** — the production composition or a supported external contract reaches the behaviour even when no direct UI sequence exists. Record the call path and the state transition that makes it reachable.
3. **Architectural leverage** — a current refactor has a concrete payoff: it removes duplicated policy, assigns one owner, closes a permissive contract, deepens a module or replaces shotgun surgery with a stable seam. Name the current owners and consumers, the proposed owner and the verification boundary.

Severity is assigned only after admission. Probability may lower priority, but a rare sequence made entirely of supported actions remains reachable.

## Evidence bar

Every finding must state:

```markdown
Admission route:
Reachability or current consumers:
Evidence:
Current impact:
Owning module:
Recommended change:
Verification boundary:
```

A user-visible defect needs a deterministic reproduction at the closest public workflow boundary. When a deterministic reproduction cannot yet be built, report the item under `Unproven hypotheses` with the missing evidence instead of assigning P0–P3.

An architecture finding does not need a user reproduction, but it must identify current code and measurable leverage. “This API could be misused someday” is not architectural leverage by itself.

## Invariant-protected claims

Call a state unreachable only when all three are named:

- the invariant that excludes it;
- its normative product or architecture source;
- an executable boundary, type or test that enforces it.

If the source exists but enforcement does not, the claim is a latent risk, not an impossibility. If the product later changes the invariant, reopen the prior decision before adding runtime recovery.

Do not add defensive runtime machinery for an invariant-protected state. Preserve the invariant and its regression test instead.

## Prior decisions

Before reporting a finding, search by domain concept rather than wording:

1. tracker issues with `Status: wontfix`;
2. `.out-of-scope/*.md`;
3. the relevant ADR and design document;
4. existing tests at the public boundary.

Reconcile a matching decision explicitly. A changed reconsideration trigger reopens triage; otherwise, cite the existing decision and do not report a duplicate finding.

## Disposition

- A confirmed defect becomes a `bug` ticket with `Status: ready-for-agent` or `ready-for-human`.
- A concrete architecture opportunity becomes an `enhancement` ticket with the appropriate ready state.
- A rejected bug claim remains a tracker ticket with `Status: wontfix`, its invariant and its reconsideration trigger. It does not enter `.out-of-scope/`.
- A durably rejected enhancement uses both a `wontfix` tracker record and one concept file under `.out-of-scope/`.
- A capability not yet present in the production Core belongs to its owning feature ticket. Missing end-to-end coverage for that future capability is not a current product defect.

All AI-authored triage comments retain the disclaimer required by `triage`.

## Review output

Present results in this order:

1. `Concrete findings` — admitted defects, ordered by severity.
2. `Architectural opportunities` — admitted refactors, ordered by leverage.
3. `Invariant-protected or previously rejected` — reconciled decisions, without severity.
4. `Unproven hypotheses` — evidence still required, without severity.

A review is complete when every reported P0–P3 satisfies one admission route and every inspected but excluded claim has a recorded disposition or a stated evidence gap.
