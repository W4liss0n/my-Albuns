---
status: current
document: research
date: 2026-09-08
ticket: 20
platform: windows
---

# Troca de conteúdo no Modo normal

Arrastar uma Foto sobre outro Frame troca as ocorrências completas, inclusive
entre Lâminas do mesmo Projeto. Um placeholder recebe a Foto e deixa a origem
vazia. A geometria, a ordem e o estilo dos Frames permanecem iguais; vínculos,
Pan e Zoom acompanham cada ocorrência. A seleção simples existente é preservada.

O gesto começa após o limiar de arraste configurado no Windows. O Core resolve
qual Frame está sob o ponteiro, respeitando a Pilha visual e a superfície ativa.
O Canvas destaca esse destino com contorno e tonalidade azuis e apresenta uma
miniatura semitransparente do recorte da Foto acompanhando o ponteiro. A
composição confirmada permanece no lugar. O cursor fica como mão fechada ao
longo do gesto e a consulta do próximo ponto não apaga o destaque confirmado.
É possível navegar com a roda ou pela rolagem automática nas bordas.
`Alt` + arraste continua a editar o Pan. `Esc`, perda de foco, cancelamento do
ponteiro e mudança da composição encerram o arraste sem Histórico. Soltar sobre
a origem, uma área vazia, uma Página inativa ou fora do Canvas também não altera
o Projeto.

A resolução final do destino e `SwapFrameContents` entram juntos na fila de
mutações. Salvar e Desfazer chamados logo depois aguardam essa operação. Um
comando anterior que altere a Foto de origem cancela a soltura pendente. A troca
continua sendo uma única ação de Histórico e usa o mesmo Salvamento e a mesma
composição de Exportação já existentes.

A seleção múltipla e os menus de troca permanecem no Modo de edição. O botão
`Trocar Frames` da Barra da Lâmina trata da troca de posições entre Páginas e
continua pertencendo à sua entrega específica. O travamento persistente de
Layout e os outros comandos pendentes continuam acompanhados pela issue #20.

## Contratos e validação

- PixiJS instalado: 8.19.0. A documentação oficial da série 8.x sobre
  [eventos](https://pixijs.com/8.x/guides/components/events) fundamenta o início
  pelo `pointerdown`; captura DOM, soltura e cancelamento seguem o padrão já
  exercitado pelo gesto de geometria do Canvas.
- Os testes públicos do Core verificam duas Fotos, Foto/placeholder, ajustes,
  Histórico, Salvamento e reabertura entre Lâminas. O corpus usado pela prévia é
  produzido e conferido por esses testes, incluindo sondas do alvo de soltura.
- Os testes da sessão de arraste verificam limiar, foco, cancelamento, resultados
  tardios, rolagem e bloqueios. Os testes do controlador cobrem a resolução
  pendente seguida por Salvar/Desfazer, sucesso, falha e mudança da origem.
- A aceitação visual usa arrastes reais sobre o Canvas no navegador sem janela
  visível. A prévia fornece coordenadas de apresentação; não substitui o gesto
  por uma chamada direta ao comando.
- `npm run test:normal-frame-swap` exercita clique, duplo clique, Pan com Alt,
  rolagem até um destino inicialmente fora da tela, cancelamento por Esc e
  estabilidade do cursor e do feedback durante movimentos entre destinos.
- O ghost usa uma única textura temporária do viewport da Foto, limitada ao
  tamanho da miniatura. Ela permanece válida se a Lâmina de origem sair da área
  materializada e é liberada ao encerrar o gesto. O contrato de
  [geração de texturas](https://pixijs.com/8.x/guides/components/renderers) da série
  8.x e as declarações instaladas do PixiJS 8.19.0 fundamentam a captura do recorte.
