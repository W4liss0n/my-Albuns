# Domain Docs

This repository uses a single domain context.

## Before working

- Read `CONTEXT.md` for domain vocabulary.
- Read the accepted ADRs relevant to the change.
- Read the canonical product specification before changing product behaviour.
- Read the linked design documents and delivery ticket for the area being changed.
- For visual UI work, use
  `docs/references/ui-programa-diagramacao/README.md` and its linked package as
  the only visual reference. Historical research fixtures are not UI references.

## Normative ownership

The sources have different responsibilities:

1. an accepted ADR owns an architectural or otherwise hard-to-reverse decision within its stated scope;
2. the canonical specification owns observable product behaviour;
3. a linked design document details an interface or technical contract without redefining either source above;
4. a delivery ticket owns the executable scope and acceptance criteria derived from those sources.

`CONTEXT.md` owns meanings and preferred names only. Research documents are historical evidence and are not normative.

A lower-level source may add detail but may not contradict a higher-level source. If sources conflict, stop implementation and reconcile the owning documents instead of choosing one silently.

## Vocabulary

Use terms exactly as defined in `CONTEXT.md`, including capitalization of concepts such as Projeto, Álbum, Lâmina, Página, Frame, Foto, Layout and Exportação.

Do not replace terms with synonyms that the glossary explicitly marks as undesirable.
