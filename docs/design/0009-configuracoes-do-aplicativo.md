---
status: accepted
document: design
---

# Configurações do aplicativo

## Objetivo

`Configurações` concentra preferências globais do MyAlbuns sem misturá-las com `Design do Álbum` ou com o estado criativo de um Projeto.

Existe somente uma janela de Configurações por instância do aplicativo. Ela pode ser aberta pela Tela de Boas-vindas ou por `Ferramentas > Configurações` em qualquer Janela de Projeto; uma nova solicitação apenas focaliza a janela existente.

Na primeira versão, a janela contém:

- `Desempenho`;
- `Outros`, que reúne as integrações, começando pelo Photoshop.

As preferências pertencem ao usuário, não participam de Undo/Redo e não exigem `Salvar` no Projeto. Escolhas simples são persistidas imediatamente; ações destrutivas ou demoradas mantêm confirmação e progresso próprios.

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

A primeira versão não expõe calibração, número de processos, threads, memória ou paralelismo. A aba apresenta somente `Cache dos álbuns`, em uma seção compacta com largura de conteúdo limitada a 480 pixels lógicos. Uma única linha reúne o espaço ocupado e o botão `Limpar cache`. Não há uma ação separada para álbuns fechados, cartões de indicadores, números em destaque nem botão `Atualizar`; os dados são consultados ao abrir a janela e quando ela recupera o foco.

```text
┌──────────────────────────────────────────────────────────────────┐
│  Cache dos álbuns                                                │
├──────────────────────────────────────────────────────────────────┤
│  Espaço ocupado    {total}        [ Limpar cache ]                │
└──────────────────────────────────────────────────────────────────┘
```

`Limpar cache` solicita a limpeza completa e nunca remove Cache pertencente a um Projeto aberto durante a execução normal. Depois da confirmação, o aplicativo executa imediatamente se não houver Projeto ou Processador ativo; caso contrário, agenda automaticamente para a próxima inicialização, antes da abertura de Projetos. O usuário não precisa escolher o alcance ou o momento da limpeza. Projetos, itens do Painel, vínculos e Arquivos originais permanecem intactos. Não existe limpeza ao vivo de Cache ativo no MVP. Quando agendada, a limpeza apresenta uma única mensagem e o botão fica desabilitado para impedir solicitações repetidas.

A confirmação ocupa a área dos indicadores, sem aumentar a janela. Ela informa o efeito da ação e a preservação dos Projetos e originais; `Cancelar` retorna aos indicadores.

O programa não impõe limite rígido, não expira Cache por idade e não mostra uma progressão de alertas por patamares arbitrários. Ele exibe o total ocupado em Configurações e avisa quando o espaço livre do volume estiver baixo, oferecendo `Liberar espaço`.

A organização física, a invalidação e as garantias do Cache estão em [Armazenamento local e Cache](0010-armazenamento-local-e-cache.md).

## Outros

A seção `Photoshop` apresenta as instalações detectadas do Adobe Photoshop. O título fica próximo às opções, sem indicação `Disponível` e sem botão `Atualizar`. O seletor `Versão utilizada` e o botão `Localizar…` ficam na mesma linha. O caminho da instalação aparece abaixo, abreviado visualmente se necessário, com o texto completo disponível para cópia e no tooltip. A ausência de instalação é informada no próprio seletor; erros de operação continuam visíveis. As instalações são consultadas ao abrir a janela e quando ela recupera o foco.

Sem preferência válida, a versão compatível mais recente começa selecionada. O usuário pode escolher outra instalação detectada ou usar `Localizar…` para indicar o executável. A mudança do nome da aba não altera o destino dos comandos existentes que abrem diretamente as preferências do Photoshop.

`Abrir no Photoshop` aparece no menu de contexto de uma Foto do Painel e de um Frame preenchido. O atalho fixo da primeira versão é `Ctrl + E`, e a ação exige exatamente uma Foto contextual.

A integração abre sempre o Arquivo vinculado original, inclusive quando ele está em UNC, unidade mapeada ou caminho longo aceito. Cache, recorte do Frame e ajustes do MyAlbuns não são incorporados. Ausência do Photoshop, Arquivo indisponível ou falha ao iniciá-lo desabilita somente aquela tentativa e não altera o Projeto.

Um Monitor de Arquivos consolida eventos rápidos e os trata como indícios. Depois que o original volta a ficar estável e legível, uma inspeção autoritativa confirma o estado; o `MediaRuntime` registra a observação e o `CacheEngine` invalida somente a representação afetada. Inexistência confirmada sob uma origem acessível produz `Arquivo ausente`, indisponibilidade de rede preserva o vínculo como `Arquivo indisponível`, e o retorno ao mesmo caminho restaura a referência sem criar Undo/Redo.

```text
┌──────────────────────────────────────────────────────────────────┐
│  Photoshop                                                       │
├──────────────────────────────────────────────────────────────────┤
│  Versão utilizada                                                │
│  [ Adobe Photoshop 2026                 ▾ ]   [ Localizar… ]      │
│  C:\...\Adobe Photoshop 2026\Photoshop.exe                       │
└──────────────────────────────────────────────────────────────────┘
```

A janela abre com 720 × 440 pixels lógicos, usa o cabeçalho, as cores e os controles compartilhados do projeto e mantém as abas e o rodapé fixos. Segue a densidade das janelas auxiliares: títulos de seção de 13 pixels, controles de 31 pixels e texto de controle de 12,5 pixels, com 12 pixels entre título e conteúdo. Conteúdo excedente rola somente na área central. Textos introdutórios redundantes são omitidos; erros e confirmações necessários continuam visíveis. Em larguras reduzidas, os espaçamentos se ajustam e as colunas podem se empilhar, sem cortar controles.

## Comandos e associações

O MVP usa atalhos e modificadores fixos e visíveis. Internamente, o `CommandCatalog` mantém identificador, descrição, contexto e associação padrão estáveis, para que menus e dicas não codifiquem combinações divergentes. Foco, seleção, reconhecimento de gestos e dispatch permanecem nos contextos da interface que os possuem; o catálogo não se torna um estado global de interação.

A interface para remapear teclado fica adiada. Quando for priorizada, deve reutilizar esses identificadores e começar por atalhos de teclado; remapeamento de modificadores de gestos só será considerado depois de testes de ambiguidade e acessibilidade.
