---
status: current
document: research
date: 2026-09-16
ticket: 12
---

# Conferência de comandos e atalhos

A etapa [Programa 34 — Registro de comandos, atalhos e modificadores do MVP](https://github.com/W4liss0n/my-Albuns/issues/12) reutiliza o catálogo existente. O catálogo possui os descritores; cada superfície continua responsável por foco, seleção, elegibilidade e execução. Não foi criada interface de remapeamento nem registro de gestos.

## Alterações

- Os comandos de importação, religação, substituição e organização em pastas agora possuem IDs, descrições, tipos e contextos no catálogo. Os menus e o botão de criação de pasta usam esses descritores, preservando os nomes existentes.
- `Enter` para editar a Lâmina passou a consultar o catálogo e respeita os modificadores declarados. `Ctrl+Enter`, por exemplo, não é tratado como `Enter` simples.
- As instruções de teclado do Canvas consultam as mesmas associações de Zoom, Ajustar Lâmina, Selecionar tudo e Editar Lâmina. Modificadores de arraste e roda continuam com os módulos de gestos.
- `Delete` com um menu de Lâmina aberto não alcança o comando implícito da Lâmina centralizada. Antes, abrir o menu da Lâmina 03 com a 02 centralizada e pressionar `Delete` podia excluir a 02. O clique em Excluir continua usando o alvo explícito do menu.
- No Painel de imagens, menus e controles conservam as teclas de edição. `Ctrl+A`, `Delete` e `Ctrl+E` não executam ações por trás de um menu aberto. Um evento já consumido também não é reinterpretado pelo Painel.

`ownsEditingKeys` compartilha apenas a identificação dos controles que possuem essas teclas. Não armazena foco ou seleção e não escolhe comandos. A arbitragem de contextos permanece nos donos atuais.

## Cobertura dos critérios

| Critério | Implementação e evidência |
|---|---|
| Selecionar Frames por `Ctrl+A` e Editar | Entrega anterior preservada; testes de `ProjectWorkspace`, `useProjectCommandShortcuts` e seleção por Caixa. |
| IDs, descrição, contexto e associação estáveis | `projectCommandCatalog.ts`; teste percorre os descritores e verifica completude, unicidade e conflitos por contexto. |
| Menus, dicas e testes usam descritores | Menus de aplicação, Frames, Lâminas e mídias; controles de orientação; instruções do Canvas. Textos de estado e opções parametrizadas de seletores continuam pertencendo à própria superfície. |
| Associações fixas, inclusive Photoshop | Testes do catálogo, `GlobalShell`, `useProjectCommandShortcuts` e `MediaPanel` cobrem comandos globais e `Ctrl+E` para exatamente uma Foto contextual. |
| `Esc` reservado | Não há associação remapeável no catálogo. Os donos de menu, diálogo, controle, gesto e modo continuam consumindo o cancelamento local antes da saída do modo. |
| Elegibilidade e ausência de execução duplicada | Testes de foco de Frames, Painel, campos, menus, diálogos, comandos bloqueados e repetição. Regressões de menus reproduzidas antes das correções e aprovadas depois. |
| Gestos permanecem locais | `FrameInteractionSession`, `FrameAreaSelectionSession`, `EditingCanvasNavigation`, sessões de Foto e arraste de mídias continuam reconhecendo seus próprios gestos. |
| Consistência entre janelas | `test:command-windows` abre dois contextos de janela independentes com as interfaces de Projeto e verifica Copiar/Colar, Histórico, entrada e saída do modo sem afetar a outra janela. |
| Registro interno, sem tela de atalhos | Configurações continua com Desempenho e Outros; não há lista de consulta, campos ou persistência de remapeamentos. |
| Separação preparada para evolução | IDs e associações pertencem ao catálogo; resolução contextual continua local. Nenhuma implementação especulativa de remapeamento foi adicionada. |
| Tipos de comando e Histórico distintos | Descritores distinguem aplicação, domínio e interface. Callbacks continuam acionando os intents e APIs tipados; IDs genéricos do catálogo não são gravados no Histórico do Core. |
| Testes de contexto, cancelamento e múltiplas janelas | Suítes existentes ampliadas nos limites de `ProjectWorkspace`, `MediaPanel`, controlador e catálogo; cenários visuais `command-sheet-menu-owns-delete` e `command-media-menu-owns-selection`; teste de duas janelas. |
| Personalização fora do MVP | Nenhuma alteração nos schemas de Configurações ou de Projeto, nem preferências de teclado ou modificadores. |

## Limite do teste de duas janelas

`npm run test:command-windows -- <pasta-de-evidências>` usa Edge sem janela visível, com dois contextos de janela no mesmo navegador. Cada um carrega o frontend produtivo por `workspace-preview.html`, com fixtures distintas. As entradas de teclado são enviadas pelo WebDriver, e o resultado é observado no DOM. O teste cobre o isolamento da interface e dos handlers; não é um teste adicional de foco nativo ou da barra de tarefas do Windows. Não abre uma instância visível do MyAlbuns nem acessa Projetos do usuário.

O caso de cópia usa o corpus existente do Core e a mesma área útil de 1567 × 900 da aceitação visual, pois o deslocamento físico da colagem depende da escala do Canvas. A evidência conserva versões do navegador/driver, resultados, capturas, commit e encerramento dos processos de teste.

## Contratos externos consultados

O projeto usa React e React DOM 19.2.8. A [referência de eventos do React](https://react.dev/reference/react-dom/components/common#react-event-object) e a [implementação da versão 19.2.8](https://github.com/facebook/react/blob/v19.2.8/packages/react-dom-bindings/src/events/SyntheticEvent.js) fundamentam a distinção entre consumir o evento e impedir sua propagação. As correções reutilizam `defaultPrevented`, `preventDefault` e os handlers existentes, sem alterar a delegação de eventos da biblioteca.

O teste de janelas usa os endpoints de [criação de janela](https://www.w3.org/TR/webdriver/#new-window), troca de contexto e ações de teclado do WebDriver, exercitados com a versão de Edge/driver registrada na evidência. O Context7 estava sem cota; a consulta foi feita diretamente nas fontes oficiais.
