# 28 — PDF multipágina

**What to build:** permitir exportar a seleção normal como um único PDF multipágina, reutilizando exatamente as unidades visuais, validações e ordem da Exportação de imagens.

**Blocked by:** 27 — Exportação JPEG e PNG completa.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [Exportação normal](../../../docs/design/0004-exportacao-normal.md); [ADR 0006 — publicação com transação limitada](../../../docs/adr/0006-publicar-exportacao-com-transacao-limitada.md).

- [ ] A tela permite PDF para Álbum inteiro ou Intervalo e mantém os modos `Por lâmina` e `Por página`.
- [ ] Ao selecionar PDF, não mostrar slider de qualidade na primeira versão.
- [ ] O arquivo se chama `{nome-do-projeto}.pdf` e contém uma página por unidade ativa na ordem definida.
- [ ] Dimensões físicas de cada página do PDF correspondem à Lâmina, à Página ou à superfície única ativa conforme o modo.
- [ ] Composição, recorte central, transparência resolvida, perfis e leitura dos originais seguem o mesmo pipeline canônico.
- [ ] Validação, destino, conflito, progresso, cancelamento e falhas seguem as garantias da Exportação normal.
- [ ] Estado visível não salvo é usado sem alterar o Projeto.
- [ ] Testes comparam páginas do PDF a composições canônicas equivalentes de JPEG/PNG.
