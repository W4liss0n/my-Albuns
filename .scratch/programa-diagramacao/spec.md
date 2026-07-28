# Programa de diagramação de álbuns

**Canonical specification:** [Programa de diagramação de álbuns](../../docs/specs/programa-de-diagramacao-de-albuns.md)

**Status:** ready-for-agent

## Delivery

The approved implementation map is published as one vertical-slice ticket per file under [`issues/`](issues/). Blocking edges define the work frontier.

Ticket 37 was added after the original numbering to own the cross-cutting Windows path contract. It is a foundation ticket: after ticket 01 validates the architecture, ticket 37 precedes tickets 02 and 04 despite its higher number.

The user interface is part of the product behaviour, not a final presentation phase. Ticket 05 defines the application screen map and interaction architecture; every subsequent functional ticket must expose its behaviour through the real interface, including the relevant empty, loading, confirmation and error states.

Tickets remain self-contained delivery briefs, but do not replace the canonical specification or accepted ADRs. Each ticket declares its work `Type` and `Normative sources`.

## Shared completion rule

For a ticket with `Type: implementation`, an internal model alone is not complete. Its behaviour must be reachable through the application interface, covered at the public workflow boundary, persisted when applicable, and reflected by Exportação when it affects visual output.

Tickets of type `spike`, `decision` or `design` are complete when their own evidence and deliverables satisfy their acceptance criteria; they are not required to ship production persistence or Exportação unless the ticket explicitly says so.
