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
- `Photoshop`.

As preferências pertencem ao usuário, não participam de Undo/Redo e não exigem `Salvar` no Projeto. Escolhas simples são persistidas imediatamente; ações destrutivas ou demoradas mantêm confirmação e progresso próprios.

```text
┌──────────────────────────────────────────────────────────────────┐
│  Configurações                                                   │
├──────────────────────────────────────────────────────────────────┤
│  Desempenho  │  Photoshop                                       │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│                 conteúdo da aba selecionada                      │
│                                                                  │
├──────────────────────────────────────────────────────────────────┤
│                                                    Fechar        │
└──────────────────────────────────────────────────────────────────┘
```

## Desempenho

A primeira versão não expõe calibração, número de processos, threads, memória ou paralelismo. A aba apresenta somente o uso do Cache e as ações seguras para liberar espaço.

```text
┌──────────────────────────────────────────────────────────────────┐
│  Cache                                                           │
├──────────────────────────────────────────────────────────────────┤
│  Espaço ocupado                        {espaço calculado}          │
│  Liberável de Projetos fechados         {espaço calculado}         │
│                                                                  │
│  [ Liberar espaço ]       [ Limpar todo o Cache ]                 │
└──────────────────────────────────────────────────────────────────┘
```

`Liberar espaço` informa quanto pode remover e, depois da confirmação, exclui somente o Cache de Projetos fechados e sem proprietário ativo. Projetos, itens do Painel, vínculos e Arquivos originais permanecem intactos.

`Limpar todo o Cache` nunca remove Cache pertencente a um Projeto aberto durante a execução normal. Se não houver Projeto ou Processador ativo, pode executar imediatamente; caso contrário, oferece agendar a limpeza para a próxima inicialização do aplicativo, antes da abertura de Projetos. Não existe limpeza ao vivo de Cache ativo no MVP.

O programa não impõe limite rígido, não expira Cache por idade e não mostra uma progressão de alertas por patamares arbitrários. Ele exibe o total ocupado em Configurações e avisa quando o espaço livre do volume estiver baixo, oferecendo `Liberar espaço`.

A organização física, a invalidação e as garantias do Cache estão em [Armazenamento local e Cache](0010-armazenamento-local-e-cache.md).

## Photoshop

A aba apresenta as instalações detectadas do Adobe Photoshop e a disponibilidade da integração.

Sem preferência válida, a versão compatível mais recente começa selecionada. O usuário pode escolher outra instalação detectada ou usar `Localizar Photoshop...` para indicar o executável.

`Abrir no Photoshop` aparece no menu de contexto de uma Foto do Painel e de um Frame preenchido. O atalho fixo da primeira versão é `Ctrl + E`, e a ação exige exatamente uma Foto contextual.

A integração abre sempre o Arquivo vinculado original, inclusive quando ele está em UNC, unidade mapeada ou caminho longo aceito. Cache, recorte do Frame e ajustes do MyAlbuns não são incorporados. Ausência do Photoshop, Arquivo indisponível ou falha ao iniciá-lo desabilita somente aquela tentativa e não altera o Projeto.

Um Monitor de Arquivos consolida eventos rápidos e os trata como indícios. Depois que o original volta a ficar estável e legível, uma inspeção autoritativa confirma o estado; o `MediaRuntime` registra a observação e o `CacheEngine` invalida somente a representação afetada. Inexistência confirmada sob uma origem acessível produz `Arquivo ausente`, indisponibilidade de rede preserva o vínculo como `Arquivo indisponível`, e o retorno ao mesmo caminho restaura a referência sem criar Undo/Redo.

```text
┌──────────────────────────────────────────────────────────────────┐
│  Photoshop                                                       │
├──────────────────────────────────────────────────────────────────┤
│  Instalação usada                                                │
│  [ Adobe Photoshop 2026                              ▾ ]          │
│  C:\...\Adobe Photoshop 2026\Photoshop.exe                       │
│                                                                  │
│  [ Localizar Photoshop... ]                                      │
│                                                                  │
│  Status: integração disponível                                   │
└──────────────────────────────────────────────────────────────────┘
```

## Comandos e associações

O MVP usa atalhos e modificadores fixos e visíveis. Internamente, o `CommandCatalog` mantém identificador, descrição, contexto e associação padrão estáveis, para que menus e dicas não codifiquem combinações divergentes. Foco, seleção, reconhecimento de gestos e dispatch permanecem nos contextos da interface que os possuem; o catálogo não se torna um estado global de interação.

A interface para remapear teclado fica adiada. Quando for priorizada, deve reutilizar esses identificadores e começar por atalhos de teclado; remapeamento de modificadores de gestos só será considerado depois de testes de ambiguidade e acessibilidade.
