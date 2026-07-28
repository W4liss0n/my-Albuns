# 08 — Esqueleto ponta a ponta

**What to build:** entregar o primeiro fluxo real do aplicativo: abrir a interface, criar um Projeto padrão em local escolhido pelo usuário, editar seu DPI com Undo/Redo, salvar, reabrir e exportar a Lâmina visível como JPEG.

**Blocked by:** 02 — Documento de Projeto e identidade; 04 — Renderizador final; 05 — Arquitetura de UI, mapa de telas e interação do editor.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [Tela de Boas-vindas](../../../docs/design/0002-tela-de-boas-vindas.md); [criação de Projeto](../../../docs/design/0003-criacao-de-projeto.md); [Exportação normal](../../../docs/design/0004-exportacao-normal.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] A tela inicial permite iniciar a criação; o fluxo do produto possui exatamente duas etapas, `Dimensões` e `Personalização`, com as configurações iniciais pertinentes a esta fatia.
- [ ] A etapa `Dimensões` contém Unidade, largura/altura da Lâmina, DPI, quantidade, extremidades, Sangria e segurança; `Personalização` contém Background, Overlay e Padrão dos Frames.
- [ ] Em `Dimensões`, agrupar Documento, Estrutura e Áreas técnicas e atualizar um resumo textual de Lâmina, Página e DPI, sem segunda prévia gráfica.
- [ ] Bloquear `Próximo` quando houver valor inválido, mostrar cada erro inline e focalizar o primeiro campo problemático sem abrir modal genérico.
- [ ] A etapa `Personalização` possui uma reprodução visual de Lâmina com Frames demonstrativos, atualizada imediatamente pelos três padrões e nunca copiada como composição inicial.
- [ ] A reprodução usa a proporção física escolhida, mas conserva os dois lados ativos para demonstrar todos os escopos visuais independentemente das extremidades configuradas.
- [ ] A reprodução permite selecionar por hover e clique lado esquerdo, lado direito ou `Ambos os lados`, e os controles de Background/Overlay atuam no escopo fixado.
- [ ] Manter a reprodução à esquerda e os controles à direita, com navegação fixa no rodapé mesmo quando a região de controles precisar de rolagem.
- [ ] `Escolher imagem...` usa o seletor nativo para Background ou Overlay; a seleção é provisória e só entra em `Decorativos` depois da criação bem-sucedida.
- [ ] Ao confirmar `Criar`, um diálogo nativo solicita local e Nome; cancelar não cria arquivo e retorna ao diálogo do produto com todas as configurações previamente preenchidas.
- [ ] O editor apresenta o shell, uma Lâmina padrão e os comandos globais definidos no mapa de telas.
- [ ] Alterar DPI participa de Undo/Redo, marca a sessão como não salva e só altera o arquivo após `Salvar`.
- [ ] Fechar e reabrir restaura o estado salvo, enquanto alterações não salvas permanecem fora do documento.
- [ ] Fechar com mudanças pendentes oferece `Salvar e fechar`, `Descartar e fechar` ou `Cancelar`.
- [ ] Undo/Redo continua disponível depois de `Salvar` e é encerrado quando a sessão é fechada.
- [ ] A Lâmina visível pode ser exportada como um JPEG com dimensões coerentes com dimensão física e DPI.
- [ ] O fluxo atravessa somente as interfaces externas do `ProjectCore` e do `ExportPipeline`; o frontend e o teste ponta a ponta não chamam diretamente as subdivisões internas.
- [ ] O fluxo completo possui teste automatizado na fronteira pública e comandos reproduzíveis de build, teste e execução.
