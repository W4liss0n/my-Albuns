---
status: accepted
document: design
updated: 2026-09-24
---

# Configurações do aplicativo

## Objetivo

`Configurações` concentra preferências globais do MyAlbuns sem misturá-las com `Design do Álbum` ou com o estado criativo de um Projeto.

Existe somente uma janela de Configurações por instância do aplicativo. Ela pode ser aberta pela Tela de Boas-vindas ou por `Ferramentas > Configurações` em qualquer Janela de Projeto; uma nova solicitação apenas focaliza a janela existente.

O título mostra somente `Configurações`, sem a marca MyAlbuns, com o controle de fechar à direita. Enquanto ela estiver aberta, as demais janelas do programa ficam bloqueadas, inclusive para fechar o Projeto. Ao fechar Configurações, o programa volta a responder; uma falha no processo que hospeda essa janela também libera o bloqueio.

Na primeira versão, a janela contém:

- `Desempenho`;
- `Outros`, que reúne as integrações, começando pelo Photoshop.

As preferências pertencem ao usuário, não participam de Undo/Redo e não exigem `Salvar` no Projeto. Escolhas simples são persistidas imediatamente. A limpeza do Cache mantém sua confirmação e, depois de autorizada, acontece em segundo plano, sem diálogo de progresso nem controle de cancelamento durante a execução.

```text
┌──────────────────────────────────────────────────────────────────┐
│  Configurações                                                   │
├──────────────────────────────────────────────────────────────────┤
│  Desempenho  │  Outros                                          │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│                 conteúdo da aba selecionada                      │
│                                                                  │
├──────────────────────────────────────────────────────────────────┤
│                                                    Fechar        │
└──────────────────────────────────────────────────────────────────┘
```

## Desempenho

A primeira versão não expõe calibração, número de processos, threads, memória ou paralelismo. A aba apresenta somente `Prévias temporárias`, em uma seção compacta. O rótulo `Espaço ocupado` fica sobre o total, e o botão `Limpar prévias` fica à direita na mesma faixa, usando toda a largura útil da janela, respeitadas as margens do conteúdo. Abaixo, uma frase curta explica o que são as prévias e o que a limpeza preserva: `Cópias reduzidas das fotos que deixam os álbuns mais rápidos. Limpar não altera projetos nem fotos originais.` Não há uma ação separada para álbuns fechados, cartões de indicadores, números em destaque nem botão `Atualizar`; os dados são consultados ao abrir a janela e quando ela recupera o foco.

```text
┌──────────────────────────────────────────────────────────────────┐
│  Prévias temporárias                                              │
├──────────────────────────────────────────────────────────────────┤
│  Espaço ocupado                                                   │
│  {total}                                  [ Limpar prévias ]      │
│  Cópias reduzidas das fotos… Limpar não altera projetos…          │
└──────────────────────────────────────────────────────────────────┘
```

`Limpar prévias` solicita a limpeza completa e nunca remove Cache pertencente a um Projeto aberto durante a execução normal. Depois da confirmação, o aplicativo executa imediatamente se não houver Projeto ou Processador ativo; caso contrário, agenda automaticamente para a próxima inicialização, antes da abertura de Projetos. O usuário não precisa escolher o alcance ou o momento da limpeza. Projetos, itens do Painel, vínculos e Arquivos originais permanecem intactos. Não existe limpeza ao vivo de Cache ativo no MVP. Quando agendada, a limpeza apresenta uma única mensagem e o botão fica desabilitado para impedir solicitações repetidas.

A confirmação aparece em um balão ancorado ao botão `Limpar prévias`, mantendo os indicadores visíveis e sem deslocar o conteúdo ou aumentar a janela. O único texto é `As prévias serão recriadas quando necessário. Os álbuns podem demorar mais para abrir.`, seguido de `Cancelar` e `Confirmar`. Cancelar, pressionar Escape ou sair do balão sem confirmar apenas fecha a confirmação; não solicita a limpeza.

O programa não impõe limite rígido, não expira Cache por idade e não mostra alertas preventivos por espaço livre ou estimativa de tamanho. Ele exibe o total ocupado em Configurações. O aviso de falta de espaço aparece somente quando a criação, gravação, finalização ou publicação de um arquivo realmente falha por esse motivo; orienta a liberar espaço e tentar novamente, sem iniciar limpeza automática.

A organização física, a invalidação e as garantias do Cache estão em [Armazenamento local e Cache](0010-armazenamento-local-e-cache.md).

## Outros

A seção `Photoshop` apresenta as instalações detectadas do Adobe Photoshop em um único seletor, `Versão utilizada`, sem indicação `Disponível` e sem botão `Atualizar`. `Localizar…` fica à direita do cabeçalho da seção, como ação discreta. O caminho da instalação escolhida aparece abaixo do seletor, abreviado visualmente se necessário, com o texto completo disponível para cópia e no tooltip. Uma frase curta indica onde a escolha é usada: `Usado em Abrir no Photoshop (Ctrl+E).`, com o nome e o atalho do catálogo de comandos. A ausência de instalação é informada no próprio seletor. As instalações são consultadas ao abrir a janela e quando ela recupera o foco.

Erros de operação não usam caixa de aviso. A mensagem abre o tooltip de validação compartilhado junto do controle cuja ação falhou: `Localizar…`, o seletor ou `Limpar prévias`. O tooltip fica fora do fluxo e não desloca a composição; fecha ao clicar fora e reabre com o foco ou o clique no controle. Enquanto o erro vale, o controle permanece marcado como inválido e a mensagem continua anunciada por leitores de tela. Uma nova tentativa limpa o erro.

Sem preferência válida, a versão compatível mais recente começa selecionada. O usuário pode escolher outra instalação detectada ou usar `Localizar…` para indicar o executável. A mudança do nome da aba não altera o destino dos comandos existentes que abrem diretamente as preferências do Photoshop.

`Abrir no Photoshop` aparece no menu de contexto de uma Foto do Painel e de um Frame preenchido. O atalho fixo da primeira versão é `Ctrl + E`, e a ação exige exatamente uma Foto contextual.

A integração abre sempre o Arquivo vinculado original, inclusive quando ele está em UNC, unidade mapeada ou caminho longo aceito. Cache, recorte do Frame e ajustes do MyAlbuns não são incorporados. Ausência do Photoshop, Arquivo indisponível ou falha ao iniciá-lo desabilita somente aquela tentativa e não altera o Projeto.

Um Monitor de Arquivos consolida eventos rápidos e os trata como indícios. Depois que o original volta a ficar estável e legível, uma inspeção autoritativa confirma o estado; o `MediaRuntime` registra a observação e o `CacheEngine` invalida somente a representação afetada. Inexistência confirmada sob uma origem acessível produz `Arquivo ausente`, indisponibilidade de rede preserva o vínculo como `Arquivo indisponível`, e o retorno ao mesmo caminho restaura a referência sem criar Undo/Redo.

```text
┌──────────────────────────────────────────────────────────────────┐
│  Photoshop                                          Localizar…   │
├──────────────────────────────────────────────────────────────────┤
│  Versão utilizada                                                │
│  [ Adobe Photoshop 2026 · 27.10          ▾ ]                      │
│  C:\...\Adobe Photoshop 2026\Photoshop.exe                       │
│  Usado em Abrir no Photoshop (Ctrl+E).                           │
└──────────────────────────────────────────────────────────────────┘
```

A janela abre com 720 × 440 pixels lógicos, usa o cabeçalho, as cores e os controles compartilhados do projeto e mantém as abas e o rodapé fixos. O botão X permanece no canto superior direito, inclusive em larguras reduzidas e quando o status do cabeçalho está oculto. Segue a densidade das janelas auxiliares: títulos de seção de 13 pixels, controles de 31 pixels e texto de controle de 12,5 pixels. Conteúdo excedente rola somente na área central. Textos introdutórios redundantes são omitidos; cada seção admite no máximo uma frase curta que explique o item ou a consequência da ação. Confirmações necessárias continuam visíveis. Em larguras reduzidas, os espaçamentos se ajustam e o botão pode passar para baixo do valor, sem cortar controles.

### Refinamento de 24/09/2026

Decisão do autor, validada em protótipo, para dar corpo à janela na linguagem
dos painéis do editor, sem cartões flutuantes nem ícones decorativos:

- as abas ficam em uma faixa de tom (`--ui-surface-muted`) com 40 pixels de
  altura. A aba selecionada usa a superfície do conteúdo, um traço azul de
  2 pixels no topo e bordas laterais finas, como se abrisse para o conteúdo;
  as demais ficam em cinza secundário e escurecem no hover;
- cada seção começa com uma faixa de cabeçalho de ponta a ponta
  (`--ui-panel-surface`, 40 pixels, borda inferior), com o título à esquerda e
  sua ação à direita, como nas seções do Painel contextual. O conteúdo tem
  margens de 24 pixels e 12 pixels entre os itens;
- valores lidos usam rótulo sobre valor, com o valor em fonte monoespaçada,
  como as medidas do editor. Caminhos de instalação também usam fonte
  monoespaçada;
- o rodapé repete o tom da faixa das abas, fechando a composição.

## Comandos e associações

O MVP usa atalhos e modificadores fixos e visíveis. Internamente, o `CommandCatalog` mantém identificador, descrição, contexto e associação padrão estáveis, para que menus e dicas não codifiquem combinações divergentes. Foco, seleção, reconhecimento de gestos e dispatch permanecem nos contextos da interface que os possuem; o catálogo não se torna um estado global de interação.

A interface para remapear teclado fica adiada. Quando for priorizada, deve reutilizar esses identificadores e começar por atalhos de teclado; remapeamento de modificadores de gestos só será considerado depois de testes de ambiguidade e acessibilidade.
