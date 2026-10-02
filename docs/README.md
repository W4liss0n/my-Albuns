# Documentação

Mapa das fontes do produto: o que cada pasta guarda, qual fonte decide o quê e por
onde começar em cada assunto.

## Ordem de autoridade

1. Um ADR aceito decide o que é difícil de reverter, dentro do seu escopo.
2. A [especificação funcional](specs/programa-de-diagramacao-de-albuns.md) decide o
   comportamento observável do produto.
3. Um design aceito detalha uma interface ou um contrato técnico, sem contradizer as
   duas fontes acima.
4. Um ticket no GitHub define o escopo e os critérios de aceite de uma entrega.

O [glossário](../CONTEXT.md) decide apenas nomes e significados. As pesquisas são
registro técnico e não são normativas. Uma fonte de nível inferior pode acrescentar
detalhe, nunca contradizer a superior: diante de um conflito, a implementação para
até que o documento dono seja reconciliado. As regras completas estão em
[agents/domain.md](agents/domain.md).

## Pastas

| Pasta | O que guarda | Nome dos arquivos | Normativa |
|---|---|---|---|
| [`specs/`](specs/) | especificação funcional | — | sim |
| [`adr/`](adr/) | decisões difíceis de reverter | `NNNN-decisao.md` | quando `accepted` |
| [`design/`](design/) | contratos de interface e de núcleo | `NNNN-tema.md` | quando `accepted` |
| [`research/`](research/) | medições, provas e investigações | `AAAA-MM-DD-tema.md` | não |
| [`research/artifacts/`](research/artifacts/) | dados das pesquisas | acompanha a pesquisa | não |
| [`research/fase-2-fluxo-persistente/`](research/fase-2-fluxo-persistente/) | mapa decisório da fase 2, arquivado | — | não |
| [`references/ui-programa-diagramacao/`](references/ui-programa-diagramacao/README.md) | referência visual da interface | — | ver o README da pasta |
| [`agents/`](agents/) | operação dos agentes, em inglês | — | processo |

As pesquisas numeradas de 0001 a 0037 são da fase 1 (até agosto de 2026) e mantêm
esse nome; as seguintes usam a data. Os números 0025 (design) e 0005 (pesquisa) não
existem.

## Estados

O campo `status` do frontmatter indica a situação de cada documento:

- ADRs e designs: `accepted`, `proposed` ou `superseded`;
- pesquisas: `current`, `historical` ou `superseded`.

Um design substituído continua no lugar e aponta para o substituto.

## Não renomeie nem mova

Issues do GitHub apontam para estes arquivos pelo caminho. Renomear ou mover um
documento quebra esses links: corrija o conteúdo no próprio arquivo.

## Por onde começar

| Assunto | Documentos |
|---|---|
| Comportamento do produto | [especificação](specs/programa-de-diagramacao-de-albuns.md); termos no [glossário](../CONTEXT.md) |
| Plataforma, processos e propriedade de estado | ADR 0005; designs 0012, 0037 e 0038 |
| Arquivo `.myalbuns` | ADRs 0009 e 0012; design 0051 (vigente; 0013 e 0016 foram substituídos) |
| Salvamento, identidade e cópias | ADR 0002; design 0015 |
| Arquivos vinculados, caminhos e Cache | ADRs 0001 e 0007; designs 0010, 0011, 0020, 0032, 0039 e 0052 |
| Exportação e renderização | ADRs 0003, 0004 e 0006; designs 0004, 0014, 0017 e 0019 |
| Fotos e Frames | designs 0017, 0021 a 0024, 0033 e 0048 |
| Layouts e Pastas | ADRs 0008 e 0010; designs 0026 a 0030, 0035 e 0042 |
| Dimensões | ADR 0011; design 0036 |
| Interface: telas e janelas | designs 0001 a 0009, 0018, 0031, 0034 e 0050 |
| Interface: controles e textos | designs 0040, 0041 e 0043 a 0049; [direção visual](../.interface-design/system.md); [referência visual](references/ui-programa-diagramacao/README.md) |
