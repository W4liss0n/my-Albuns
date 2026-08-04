# Esqueleto ponta a ponta

**Parent issue:** [08 — Esqueleto ponta a ponta](../programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md)

**Decision map:** [Fase 2 — Primeiro fluxo real persistente](../fase-2-fluxo-persistente/map.md)

**Status:** ready-for-agent

## Delivery

This tracker entry decomposes the approved first persistent product flow into one implementation ticket per file under [`issues/`](issues/). The parent issue remains unchanged; these children are its executable delivery path.

Tickets are numbered in dependency order. Ticket 01 expands the production contract beside the Phase 1 scaffolding, tickets 02–10 migrate complete user-visible slices, and ticket 11 contracts the obsolete form only after every consumer has moved. No prototype branch is merged into production; the approved prototype is an implementation oracle recorded by the decision map.

The broad parent tickets 02, 04, 05 and 39 remain open for work outside this first flow. Their relevant path, persistence, UI and rendering criteria are distributed among these child tickets instead of acting as all-or-nothing blockers.

## Shared completion rule

Every ticket must remain green on its own and expose its behaviour through the public boundary named by its normative sources. Product UI must depend on application ports rather than Tauri directly; persistence callers must use `ProjectCore`; Exportação callers must use `ExportPipeline`.

Tests observe user-visible state, persisted bytes, typed terminals, process ownership and generated files. They must not rely on private subdivisions merely to assert implementation details. Windows paths crossing a frontend or process boundary use the accepted reversible native-path DTO.

The temporary data namespace remains `MyAlbuns2`. `Salvar como`, Cópia externa completa, Recuperação, focus by alias, multiple simultaneous Projects, PNG/PDF, batch operations, wide-gamut conversion and the final `MyAlbuns` namespace migration remain outside this tracker.
