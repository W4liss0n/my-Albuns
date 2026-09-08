---
status: current
document: research
date: 2026-09-08
ticket: 20
platform: windows
---

# Troca de lados da Lâmina

No Modo normal, o botão de duas setas na Barra da Lâmina troca os Frames
inteiramente contidos em uma Página para a posição relativa equivalente na
Página oposta. A ação usa a Lâmina do botão, preservando seleção e navegação.
O botão também aceita ativação por teclado e apresenta foco visível.

O Core translada a coordenada horizontal pela largura de uma Página. Um Frame
que termina exatamente na divisão central pertence à Página esquerda; um que
começa nessa divisão pertence à direita. Frames com Travessia central permanecem
no lugar. A troca preserva IDs, ordem visual, dimensões, vínculos de Fotos, Pan,
Zoom e a Numeração de Página, sem espelhar conteúdo ou modificar Originais.

A operação inteira forma uma ação de Undo/Redo, persiste no Salvamento e na
reabertura e atualiza a composição da Exportação. Lâminas vazias ou contendo
somente Travessias centrais não criam Revisão, alteração ou entrada de Histórico,
nem descartam um ramo de Refazer. O esquema persistente continua v3.

O comando fica indisponível em Página única, no Modo de edição, durante uma
reordenação ou quando a Janela bloqueia interações. O Core rejeita Página única
e Lâminas inexistentes atomicamente. A fila da Janela confere o destino explícito
novamente ao executar: exclusão ou conversão prévia cancela a troca. Salvar e
Histórico aguardam a mesma fila e não ultrapassam uma troca pendente.

A Barra possui um único controle semântico sobre a região das setas; o Pixi
desenha sua aparência e recebe hover e foco dessa superfície. O gesto não inicia
reordenação, seleção, menu contextual ou entrada no Modo de edição.

Os testes públicos do Core verificam limites centrais, preservação das Fotos,
Histórico, Salvamento, reabertura e composição de Exportação. O corpus visual é
produzido e conferido por esses testes. Testes da integração exercitam a fila em
sucesso e falha, o alvo explícito, a navegação e os estados indisponíveis; o
manifesto de aceitação cobre clique, teclado, escalas e as superfícies afetadas.

O travamento persistente de Layout e estilos individuais de Frame acompanham
suas entregas na issue #20. A interface já respeita o indicador de Layout travado,
mas o Core atual ainda não oferece esse estado; não se declara essa capacidade
como entregue neste recorte.

A integração usa React 19.2.8 e PixiJS 8.19.0. Os contratos de eventos foram
conferidos na [documentação de eventos do PixiJS 8](https://pixijs.com/8.x/guides/components/events)
e na [implementação oficial de eventos do React](https://github.com/facebook/react/blob/main/packages/react-dom-bindings/src/events/getListener.js),
com o índice de documentação disponível para React 19.2.7. A verificação em
navegador usa as versões instaladas no projeto.
