# GitHub Issue Migration Manifest

Status: completed

Completed: 2026-08-05

Umbrella: [#1 — Programa de diagramação de álbuns](https://github.com/W4liss0n/my-Albuns/issues/1)

## Outcome

- 40 open GitHub issues: one umbrella and 39 executable or decision tickets.
- 37 program tickets are direct sub-issues of #1.
- Esqueleto tickets 10 and 11 are sub-issues of Programa 08 (#6).
- 65 native blocking relationships were applied and verified by readback.
- All ticket bodies, triage labels, parents and blockers matched this manifest at completion.
- At migration completion, active issues #7 and #8 named the normative files under `docs/design/0013–0015` and `docs/research/fase-2-fluxo-persistente/` as local paths pending publication. This changeset publishes those sources; after merge, the issue bodies can use canonical `main` links.

Repository: `W4liss0n/my-Albuns`

## Migration policy

- The approved local `/to-tickets` breakdown is preserved; no ticket is merged or split.
- Create one open umbrella issue for the canonical program specification.
- Create 37 open program issues as sub-issues of the umbrella.
- Create the two remaining `Esqueleto ponta a ponta` issues as sub-issues of program ticket 08.
- Preserve each ticket's `ready-for-agent` or `ready-for-human` state as its GitHub triage label.
- Do not publish completed work as closed issues: program tickets 01 and 37, Esqueleto tickets 01–09, and Wayfinder tickets 01–09 remain historical evidence.
- Program tickets 01 and 37 are treated as resolved; dependencies on them are already satisfied and will not become native GitHub blocking edges.
- Program ticket 08 uses the executable child breakdown. Its old broad blockers 02, 04, 05 and 39 remain context, not native gates.
- Esqueleto ticket 10 has no open blockers. Esqueleto ticket 11 is natively blocked by Esqueleto ticket 10.
- All other unresolved program blocking edges are migrated as native GitHub dependencies.
- The resolved Phase 2 Wayfinder map is archived under `docs/research/`, not recreated as closed GitHub issues.
- Existing local tracker files remain in place until every source link and GitHub readback has been verified.

## Proposed hierarchy

```text
Programa de diagramação de álbuns (umbrella)
├── Programa 02–36, excluding resolved 37
├── Programa 38
├── Programa 39
└── Programa 08 — Esqueleto ponta a ponta
    ├── Esqueleto 10
    └── Esqueleto 11 (blocked by Esqueleto 10)
```

## Proposed issues

1. **Programa 02 — 02 — Documento de Projeto e identidade**
   - Source: `.scratch/programa-diagramacao/issues/02-documento-de-projeto-e-identidade.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `design`
   - Blocked by: 01 — Plataforma e arquitetura; 37 — Política e resolução de caminhos Windows. (references to resolved Programa 01/37 will be omitted as native edges)
   - What it delivers: especificar o documento persistido de Projeto e o mecanismo que permite salvar com segurança, distinguir movimentações de Cópias externas, coordenar aberturas simultâneas e recuperar sessões sem salvamento automático.
   - GitHub issue: [#2](https://github.com/W4liss0n/my-Albuns/issues/2)

2. **Programa 03 — 03 — Mídias externas e Cache**
   - Source: `.scratch/programa-diagramacao/issues/03-midias-externas-e-cache.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 01 — Plataforma e arquitetura; 02 — Documento de Projeto e identidade.
   - What it delivers: decidir como Fotos e Decorativos vinculados serão identificados, decodificados e visualizados com desempenho, mantendo os arquivos originais como única fonte autorizada para operações finais.
   - GitHub issue: [#11](https://github.com/W4liss0n/my-Albuns/issues/11)

3. **Programa 04 — 04 — Renderizador final**
   - Source: `.scratch/programa-diagramacao/issues/04-renderizador-final.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `design`
   - Blocked by: 01 — Plataforma e arquitetura; 37 — Política e resolução de caminhos Windows. (references to resolved Programa 01/37 will be omitted as native edges)
   - What it delivers: especificar um pipeline determinístico que componha o estado atual do Projeto usando os originais e produza arquivos finais nas dimensões físicas esperadas, sem depender do Canvas ou do Cache de visualização.
   - GitHub issue: [#3](https://github.com/W4liss0n/my-Albuns/issues/3)

4. **Programa 05 — 05 — Arquitetura de UI, mapa de telas e interação do editor**
   - Source: `.scratch/programa-diagramacao/issues/05-arquitetura-de-ui-e-interacao.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `design`
   - Blocked by: 01 — Plataforma e arquitetura. (references to resolved Programa 01/37 will be omitted as native edges)
   - What it delivers: produzir o mapa navegável e o protótipo de interação que estabelecem a estrutura das telas, os modos do editor e os principais estados da primeira versão antes da implementação funcional.
   - GitHub issue: [#4](https://github.com/W4liss0n/my-Albuns/issues/4)

5. **Programa 06 — 06 — Gerador de Layouts**
   - Source: `.scratch/programa-diagramacao/issues/06-gerador-de-layouts.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-human`
   - Type: `decision`
   - Blocked by: 20 — Aplicação de Layouts.
   - What it delivers: decidir posteriormente o contrato do algoritmo que produzirá organizações compatíveis para a quantidade atual de Frames, depois que a aplicação de Layouts estiver validada com definições de teste.
   - GitHub issue: [#28](https://github.com/W4liss0n/my-Albuns/issues/28)

6. **Programa 07 — 07 — Transformação dimensional**
   - Source: `.scratch/programa-diagramacao/issues/07-transformacao-dimensional.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-human`
   - Type: `decision`
   - Blocked by: 01 — Plataforma e arquitetura. (references to resolved Programa 01/37 will be omitted as native edges)
   - What it delivers: definir quando uma mudança nas dimensões físicas do Projeto pode ser aplicada com segurança e como todo o conteúdo é transformado proporcionalmente sem perder o enquadramento das Fotos.
   - GitHub issue: [#5](https://github.com/W4liss0n/my-Albuns/issues/5)

7. **Programa 08 — 08 — Esqueleto ponta a ponta**
   - Source: `.scratch/programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: None as a native gate; completion is represented by its executable sub-issues
   - What it delivers: entregar o primeiro fluxo real do aplicativo: abrir a interface, criar um Projeto padrão em local escolhido pelo usuário, editar seu DPI com Undo/Redo, salvar, reabrir e exportar a Lâmina visível como JPEG.
   - GitHub issue: [#6](https://github.com/W4liss0n/my-Albuns/issues/6)

8. **Programa 09 — 09 — Primeira composição com Foto**
   - Source: `.scratch/programa-diagramacao/issues/09-primeira-composicao-com-foto.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 03 — Mídias externas e Cache; 08 — Esqueleto ponta a ponta.
   - What it delivers: permitir que o usuário importe um JPEG vinculado, coloque-o em um Frame, ajuste o enquadramento básico e obtenha a mesma composição depois de salvar, reabrir e exportar.
   - GitHub issue: [#17](https://github.com/W4liss0n/my-Albuns/issues/17)

9. **Programa 10 — 10 — Álbum físico**
   - Source: `.scratch/programa-diagramacao/issues/10-album-fisico.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 08 — Esqueleto ponta a ponta.
   - What it delivers: permitir criar e editar a estrutura física completa do Álbum, navegar por Lâminas e Páginas em escala coerente e visualizar corretamente Sangria, segurança e lados desativados.
   - GitHub issue: [#9](https://github.com/W4liss0n/my-Albuns/issues/9)

10. **Programa 11 — 11 — Salvar como e cópia explícita**
   - Source: `.scratch/programa-diagramacao/issues/11-salvar-como-e-copia-explicita.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 09 — Primeira composição com Foto.
   - What it delivers: permitir criar deliberadamente outro Projeto a partir do atual, copiando todo o estado visível e as preferências pertencentes ao Projeto sem estabelecer herança ou sincronização entre os dois.
   - GitHub issue: [#18](https://github.com/W4liss0n/my-Albuns/issues/18)

11. **Programa 12 — 12 — Movimentação e Cópia externa**
   - Source: `.scratch/programa-diagramacao/issues/12-movimentacao-e-copia-externa.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 08 — Esqueleto ponta a ponta.
   - What it delivers: reconhecer quando o arquivo de Projeto foi apenas movido e, quando uma cópia feita pelo sistema operacional for aberta, atribuir-lhe identidade própria sem exigir uma ação adicional do usuário.
   - GitHub issue: [#10](https://github.com/W4liss0n/my-Albuns/issues/10)

12. **Programa 13 — 13 — Bloqueio de abertura**
   - Source: `.scratch/programa-diagramacao/issues/13-bloqueio-de-abertura.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 12 — Movimentação e Cópia externa.
   - What it delivers: impedir edições concorrentes acidentais do mesmo Projeto, conduzindo o usuário à sessão já aberta e recuperando com segurança bloqueios deixados por encerramentos inesperados.
   - GitHub issue: [#14](https://github.com/W4liss0n/my-Albuns/issues/14)

13. **Programa 14 — 14 — Recuperação de sessão**
   - Source: `.scratch/programa-diagramacao/issues/14-recuperacao-de-sessao.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 08 — Esqueleto ponta a ponta; 12 — Movimentação e Cópia externa.
   - What it delivers: recuperar alterações de uma sessão interrompida em um estado editável separado, preservando a regra de que o arquivo do usuário só muda quando ele executa `Salvar`.
   - GitHub issue: [#15](https://github.com/W4liss0n/my-Albuns/issues/15)

14. **Programa 15 — 15 — Ciclo de mídias externas**
   - Source: `.scratch/programa-diagramacao/issues/15-ciclo-de-midias-externas.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 09 — Primeira composição com Foto.
   - What it delivers: completar o ciclo de importação e manutenção de Fotos e Decorativos vinculados, oferecendo prévias rápidas, avisos de ausência e religação controlada sem incorporar os originais ao Projeto.
   - GitHub issue: [#19](https://github.com/W4liss0n/my-Albuns/issues/19)

15. **Programa 16 — 16 — Background e Overlay**
   - Source: `.scratch/programa-diagramacao/issues/16-background-e-overlay.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 10 — Álbum físico; 15 — Ciclo de mídias externas.
   - What it delivers: permitir compor cada Lâmina com Background e Overlay por lado ou em `Ambos os lados`, usando defaults do Projeto e substituições customizadas que respondem corretamente a mudanças posteriores do padrão.
   - GitHub issue: [#21](https://github.com/W4liss0n/my-Albuns/issues/21)

16. **Programa 17 — 17 — Edição de Frames e Fotos**
   - Source: `.scratch/programa-diagramacao/issues/17-edicao-de-frames-e-fotos.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 09 — Primeira composição com Foto; 10 — Álbum físico.
   - What it delivers: oferecer edição completa da geometria de vários Frames e do enquadramento de suas Fotos, mantendo a Foto sempre recortada pela máscara retangular e sem áreas vazadas.
   - GitHub issue: [#20](https://github.com/W4liss0n/my-Albuns/issues/20)

17. **Programa 18 — 18 — Estilos e transformações**
   - Source: `.scratch/programa-diagramacao/issues/18-estilos-e-transformacoes.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 17 — Edição de Frames e Fotos.
   - What it delivers: permitir personalizar aparência do Frame e transformações não destrutivas da Foto, distinguindo corretamente defaults do Projeto de substituições locais.
   - GitHub issue: [#22](https://github.com/W4liss0n/my-Albuns/issues/22)

18. **Programa 19 — 19 — Painel de imagens**
   - Source: `.scratch/programa-diagramacao/issues/19-painel-de-imagens.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 16 — Background e Overlay; 17 — Edição de Frames e Fotos.
   - What it delivers: entregar o Painel de imagens completo, no qual Fotos e Decorativos têm importação, organização, filtros e indicadores de uso próprios sem misturar responsabilidades.
   - GitHub issue: [#24](https://github.com/W4liss0n/my-Albuns/issues/24)

19. **Programa 20 — 20 — Aplicação de Layouts**
   - Source: `.scratch/programa-diagramacao/issues/20-aplicacao-de-layouts.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 18 — Estilos e transformações.
   - What it delivers: permitir explorar e aplicar organizações compatíveis de Frames sem manter referência viva ao Layout original, preservando conteúdo e oferecendo automação previsível fora do Modo de edição.
   - GitHub issue: [#25](https://github.com/W4liss0n/my-Albuns/issues/25)

20. **Programa 21 — 21 — Layout travado**
   - Source: `.scratch/programa-diagramacao/issues/21-layout-travado.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 20 — Aplicação de Layouts.
   - What it delivers: permitir aplicar e travar um Layout diretamente pelo painel, preservando sua estrutura de Frames até que o usuário o destrave e impedindo Exportação enquanto houver posições sem Foto.
   - GitHub issue: [#26](https://github.com/W4liss0n/my-Albuns/issues/26)

21. **Programa 22 — 22 — Layouts personalizados globais**
   - Source: `.scratch/programa-diagramacao/issues/22-layouts-personalizados-globais.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 20 — Aplicação de Layouts.
   - What it delivers: permitir transformar a disposição atual de Frames em um Layout reutilizável em todos os Projetos, identificando automaticamente se sua organização pertence ao escopo de Lâmina ou Página.
   - GitHub issue: [#27](https://github.com/W4liss0n/my-Albuns/issues/27)

22. **Programa 23 — 23 — Layouts favoritos do Projeto**
   - Source: `.scratch/programa-diagramacao/issues/23-layouts-favoritos-do-projeto.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 11 — Salvar como e cópia explícita; 20 — Aplicação de Layouts; 22 — Layouts personalizados globais.
   - What it delivers: permitir favoritar um Layout dentro do Projeto como uma cópia estável e portátil, garantindo que ele viaje com o Projeto e não dependa mais do item global que o originou.
   - GitHub issue: [#29](https://github.com/W4liss0n/my-Albuns/issues/29)

23. **Programa 24 — 24 — Remoção de imagens e Decorativos**
   - Source: `.scratch/programa-diagramacao/issues/24-remocao-de-imagens-e-decorativos.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 19 — Painel de imagens; 21 — Layout travado.
   - What it delivers: permitir remover itens do Painel de imagens com consequências explícitas e consistentes em todos os seus usos, sem deixar referências quebradas ou violar um Layout travado.
   - GitHub issue: [#30](https://github.com/W4liss0n/my-Albuns/issues/30)

24. **Programa 25 — 25 — Mudança dimensional segura**
   - Source: `.scratch/programa-diagramacao/issues/25-mudanca-dimensional-segura.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 07 — Transformação dimensional; 16 — Background e Overlay; 21 — Layout travado.
   - What it delivers: permitir alterar dimensões físicas e orientação dentro dos limites seguros, transformando proporcionalmente toda a composição e recusando mudanças nas quais a qualidade do resultado não possa ser garantida.
   - GitHub issue: [#31](https://github.com/W4liss0n/my-Albuns/issues/31)

25. **Programa 26 — 26 — Conversão de extremidades**
   - Source: `.scratch/programa-diagramacao/issues/26-conversao-de-extremidades.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 16 — Background e Overlay; 21 — Layout travado.
   - What it delivers: permitir converter Lâminas inicial e final entre Lâmina dupla e página única, reorganizando o conteúdo para a superfície ativa de forma previsível e sem conservar conteúdo inacessível no lado desativado.
   - GitHub issue: [#32](https://github.com/W4liss0n/my-Albuns/issues/32)

26. **Programa 27 — 27 — Exportação JPEG e PNG completa**
   - Source: `.scratch/programa-diagramacao/issues/27-exportacao-jpeg-e-png-completa.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 04 — Renderizador final; 26 — Conversão de extremidades.
   - What it delivers: entregar a tela completa de Exportação de imagens, capaz de renderizar o Álbum inteiro ou um Intervalo como `Por lâmina` ou `Por página`, usando o estado visível atual e os Arquivos originais.
   - GitHub issue: [#35](https://github.com/W4liss0n/my-Albuns/issues/35)

27. **Programa 28 — 28 — PDF multipágina**
   - Source: `.scratch/programa-diagramacao/issues/28-pdf-multipagina.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 27 — Exportação JPEG e PNG completa.
   - What it delivers: permitir exportar a seleção normal como um único PDF multipágina, reutilizando exatamente as unidades visuais, validações e ordem da Exportação de imagens.
   - GitHub issue: [#37](https://github.com/W4liss0n/my-Albuns/issues/37)

28. **Programa 29 — 29 — Limpeza de saídas órfãs**
   - Source: `.scratch/programa-diagramacao/issues/29-limpeza-de-saidas-orfas.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 27 — Exportação JPEG e PNG completa.
   - What it delivers: ao sobrescrever uma Exportação completa de imagens, remover arquivos antigos que ainda correspondam exatamente ao namespace de Nome e formato do mesmo Projeto, evitando saídas órfãs de uma versão anterior maior.
   - GitHub issue: [#38](https://github.com/W4liss0n/my-Albuns/issues/38)

29. **Programa 30 — 30 — Geração de Projetos em lote**
   - Source: `.scratch/programa-diagramacao/issues/30-geracao-de-projetos-em-lote.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 13 — Bloqueio de abertura; 19 — Painel de imagens; 21 — Layout travado; 23 — Layouts favoritos do Projeto.
   - What it delivers: usar o Projeto aberto como modelo para gerar recursivamente Projetos completos e independentes a partir de uma árvore de pastas com imagens, preservando a hierarquia relativa no destino.
   - GitHub issue: [#36](https://github.com/W4liss0n/my-Albuns/issues/36)

30. **Programa 31 — 31 — Exportação em lote**
   - Source: `.scratch/programa-diagramacao/issues/31-exportacao-em-lote.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 13 — Bloqueio de abertura; 28 — PDF multipágina; 29 — Limpeza de saídas órfãs.
   - What it delivers: localizar recursivamente Projetos persistidos e exportar serialmente o Álbum inteiro de cada um, em Modo de lote exclusivo, preservando a hierarquia de destino e isolando os resultados por item.
   - GitHub issue: [#39](https://github.com/W4liss0n/my-Albuns/issues/39)

31. **Programa 32 — 32 — Configurações globais e Cache**
   - Source: `.scratch/programa-diagramacao/issues/32-configuracoes-globais-e-cache.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 03 — Mídias externas e Cache; 05 — Arquitetura de UI e interação.
   - What it delivers: criar a janela global de Configurações em abas e entregar uma gestão simples e segura do Cache, sem limites automáticos, calibração ou limpeza de dados pertencentes a Projetos ativos.
   - GitHub issue: [#16](https://github.com/W4liss0n/my-Albuns/issues/16)

32. **Programa 33 — 33 — Integração com Photoshop**
   - Source: `.scratch/programa-diagramacao/issues/33-integracao-com-photoshop.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 15 — Ciclo de mídias externas; 32 — Configurações globais e Cache.
   - What it delivers: detectar uma instalação compatível do Adobe Photoshop e permitir abrir uma Foto do MyAlbuns no aplicativo externo.
   - GitHub issue: [#23](https://github.com/W4liss0n/my-Albuns/issues/23)

33. **Programa 34 — 34 — Registro de comandos, atalhos e modificadores do MVP**
   - Source: `.scratch/programa-diagramacao/issues/34-registro-de-comandos-atalhos-e-modificadores.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 05 — Arquitetura de UI e interação.
   - What it delivers: manter IDs, descrições, contextos declarados e associações padrão do MVP em um `CommandCatalog` estável, sem centralizar o estado transitório ou o dispatch da interface.
   - GitHub issue: [#12](https://github.com/W4liss0n/my-Albuns/issues/12)

34. **Programa 35 — 35 — Duplicação de Lâmina**
   - Source: `.scratch/programa-diagramacao/issues/35-duplicacao-de-lamina.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 16 — Background e Overlay; 21 — Layout travado.
   - What it delivers: permitir criar, imediatamente depois da origem, uma cópia completa e independente de uma Lâmina válida, reutilizando seus vínculos externos sem sincronizar edições futuras.
   - GitHub issue: [#33](https://github.com/W4liss0n/my-Albuns/issues/33)

35. **Programa 36 — 36 — Reutilização de Frames**
   - Source: `.scratch/programa-diagramacao/issues/36-reutilizacao-de-frames.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 21 — Layout travado.
   - What it delivers: permitir reutilizar composições por cópia e colagem de Frames e trocar o conteúdo entre duas posições sem alterar a geometria de seus Frames.
   - GitHub issue: [#34](https://github.com/W4liss0n/my-Albuns/issues/34)

36. **Programa 38 — 38 — Migração do namespace temporário de dados**
   - Source: `.scratch/programa-diagramacao/issues/38-migracao-do-namespace-temporario-de-dados.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 06 — Gerador de Layouts; 14 — Recuperação de sessão; 24 — Remoção de imagens e Decorativos; 25 — Mudança dimensional segura; 30 — Geração de Projetos em lote; 31 — Exportação em lote; 33 — Integração com Photoshop; 34 — Registro de comandos, atalhos e modificadores do MVP; 35 — Duplicação de Lâmina; 36 — Reutilização de Frames; 39 — Integração de caminhos no primeiro fluxo real.
   - What it delivers: substituir as raízes temporárias `MyAlbuns2` pelas raízes finais `MyAlbuns` somente no encerramento completo da primeira versão, com uma política explícita que não misture nem sobrescreva silenciosamente dados da versão antiga, do desenvolvimento ou da versão nova.
   - GitHub issue: [#40](https://github.com/W4liss0n/my-Albuns/issues/40)

37. **Programa 39 — 39 — Integração de caminhos no primeiro fluxo real**
   - Source: `.scratch/programa-diagramacao/issues/39-integracao-de-caminhos-no-primeiro-fluxo-real.md`
   - Parent: Programa de diagramação de álbuns
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: 02 — Documento de Projeto e identidade; 04 — Renderizador final; 05 — Arquitetura de UI, mapa de telas e interação do editor; 37 — Política e resolução de caminhos Windows.
   - What it delivers: completar as primitivas de criação segura e levar a política de caminhos já provada às fronteiras reais de criação, abertura e Exportação do primeiro Projeto persistente.
   - GitHub issue: [#13](https://github.com/W4liss0n/my-Albuns/issues/13)

38. **Esqueleto 10 — 10 — Exportar a Lâmina visível com personalização e estado não salvo**
   - Source: `.scratch/esqueleto-ponta-a-ponta/issues/10-exportar-lamina-visivel-personalizada-nao-salva.md`
   - Parent: Programa 08 — Esqueleto ponta a ponta
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: None — all local blockers are resolved
   - What it delivers: exportar exatamente a Lâmina visível, incluindo Background e Overlay configurados e o DPI corrente ainda não salvo, usando os Arquivos originais sem modificar o Projeto.
   - GitHub issue: [#7](https://github.com/W4liss0n/my-Albuns/issues/7)

39. **Esqueleto 11 — 11 — Contrair o scaffolding e fechar o gate ponta a ponta**
   - Source: `.scratch/esqueleto-ponta-a-ponta/issues/11-contrair-scaffolding-e-fechar-gate.md`
   - Parent: Programa 08 — Esqueleto ponta a ponta
   - Triage label: `ready-for-agent`
   - Type: `implementation`
   - Blocked by: Esqueleto 10
   - What it delivers: deixar somente o fluxo produtivo aprovado — Boas-vindas, criação, DPI, Salvamento, fechamento, reabertura e JPEG — removendo os caminhos demonstrativos da Fase 1 e consolidando uma verificação reproduzível da jornada completa.
   - GitHub issue: [#8](https://github.com/W4liss0n/my-Albuns/issues/8)

## Historical material not published as issues

- Programa 01 — Plataforma e arquitetura: completed; local status will be corrected to `resolved`.
- Programa 37 — Política e resolução de caminhos Windows: completed; local status will be corrected to `resolved`.
- Esqueleto 01–09: completed implementation evidence retained locally.
- Fase 2 Wayfinder 01–09: resolved decisions archived under `docs/research/fase-2-fluxo-persistente/`.

## Labels to provision

- `needs-triage`
- `needs-info`
- `ready-for-agent`
- `ready-for-human`

The existing `wontfix` label is reused.
