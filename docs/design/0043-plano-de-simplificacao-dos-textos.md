---
status: accepted
document: design
date: 2026-09-18
updated: 2026-09-18
implementation-readiness: ready-for-agent
---

# Plano de simplificação dos textos da interface

## Resultado da revisão

O MyAlbuns pode falar de maneira mais simples sem perder a precisão necessária ao trabalho de diagramação. As maiores oportunidades estão em três pontos: nomes consistentes em português; erros explicados pelo efeito na tarefa; instruções que correspondam ao próximo passo realmente disponível.

Este plano foi aprovado pelo usuário em 18 de setembro de 2026, com preferência por nomes em português. Quadro, Fundo, Sobreposição e os demais nomes abaixo passam a reger a apresentação do programa. A revisão não autoriza alterar regras de salvamento, recuperação, exportação ou perda de conteúdo para fazer a frase caber.

Os exemplos do catálogo registram o código anterior, no commit `de6652521bbf4194c75da11e7df6c0657fec0084`. São 65 alterações editoriais e duas entradas mantidas, não uma lista de defeitos funcionais. As resoluções abaixo complementam as propostas condicionais.

## Resolução das condições de implementação

| Entrada | Decisão confirmada no proprietário do fluxo |
| --- | --- |
| T06 | A guarda de revisão antecede a escrita em `persistent_project.rs`. Informar que há alterações mais recentes e que nada foi salvo nesta tentativa; não orientar a reabrir ou executar um comando inexistente de atualização. |
| T07 | A verificação abrange arquivo original e destino. Orientar a conferir se o original continua no mesmo local; não prometer que trocar o destino ou repetir corrige qualquer causa. |
| T09 | `project_host.rs` consome a sessão quando o estado salvo fica indeterminado. Manter a orientação de reabrir e conferir o conteúdo, sem afirmar sucesso, falha total ou preservação de alterações ainda não confirmadas. |
| T10 | A falha de limpeza mantém o projeto já salvo. O teste de persistência confirma que salvar novamente conclui a limpeza; preservar essa orientação. |
| T11 | Um destino inválido também pode impedir Salvar como pela verificação do original. Orientar a conferir se o arquivo foi movido ou substituído; não indicar Salvar como como solução universal. |
| T52 | Os dois botões de `GenerationWindow` antecedem o início do trabalho preparado. Usar “Iniciar geração”; nenhum deles retoma um lote interrompido. |
| T65 | `side` já contém “esquerdo” ou “direito”. Usar “Quadro de exemplo {n}, lado {side}”, preservando os valores em português e sem duplicar a descrição nas bordas. |

T67 preserva também a guia central: **“Guias de dobra, corte e segurança da lâmina”**. Limites, unidades, atalhos, IDs e os proprietários de validação permanecem existentes. Erros tipados de edição ganham apresentação na fronteira nativa (`project_error_message`), com diagnóstico no registro; os canais estruturados de persistência, exportação e Photoshop conservam seus códigos e fatos.

Os nomes e textos aceitos aqui prevalecem sobre rótulos antigos nos desenhos e na referência visual. A confirmação de limpeza usa o balão existente; validações de campo continuam em tooltip. A aplicação não acrescenta parágrafos de espera aos diálogos.

## Base e cobertura

A [pesquisa de padrões de escrita](../research/2026-09-18-padroes-de-escrita-para-interfaces.md) reúne seis fontes primárias: Microsoft, dois guias da Apple, GOV.UK, W3C e Gov.br. Os princípios adotados são linguagem familiar, voz ativa, consistência, consequências explícitas e sugestões de correção conhecidas. A regra local de tooltip prevalece sobre a apresentação de erros em linha exemplificada por outros sistemas.

O inventário percorreu os cinco pontos de entrada declarados em `vite.config.ts`: projeto, boas-vindas, diálogos globais, diálogos do projeto e geração em lote. A busca estática alcançou **229 módulos TypeScript/TSX** e examinou **182 arquivos Rust**. Foram extraídos **2.988 candidatos**, sendo 1.440 do frontend e 1.548 de Rust. Esses números incluem repetições, fragmentos de frases, identificadores e erros internos. **Não são 2.988 mensagens visíveis nem 2.988 problemas.**

O extrator percorre importações relativas, textos JSX, atributos de texto e literais candidatos. No Rust, identifica literais e exclui módulos usuais de testes. Não substitui rastreamento de execução: importações dinâmicas, concatenações, mensagens do sistema e falhas de bibliotecas podem exigir inspeção adicional. Os grupos abaixo foram lidos no código; as recomendações foram confrontadas com seus componentes e consumidores.

| Superfície | Conteúdo incluído | Resultado editorial |
| --- | --- | --- |
| Boas-vindas e criação | Recentes, modelos, medidas, personalização e abertura | Orientar como começar; traduzir termos; explicar o próximo seletor de arquivo. |
| Editor | Menus, comandos, painéis, organização de imagens, layouts e propriedades | Um nome por conceito; retirar palavras de arquitetura das dicas. |
| Validação | Dimensões, resolução, sangria, zoom, ângulo, cor e limites | Preservar valores e unidades; explicar a correção no tooltip. |
| Persistência | Abrir, salvar, salvar como, fechar, recuperar e cópia externa | Distinguir trabalho não salvo, versão no disco, cópia e resultado incerto. |
| Decisões destrutivas | Remover imagens, excluir layout, converter extremidade e substituir arquivos | Manter o objeto, o alcance e a consequência da escolha. |
| Exportação e geração | Configuração, pendências, conflitos, cancelamento, falta de espaço e resultado parcial | Ação pelo nome real; preservar o que já foi concluído e o que falta. |
| Configurações | Prévias temporárias e integração com Photoshop | Explicar a limpeza e a escolha do programa sem vocabulário interno. |
| Inicialização | Bloqueio gráfico e diagnóstico | Mensagem principal simples, detalhes técnicos disponíveis para suporte. |
| Textos acessíveis | Nomes de campos, botões, prévias e valores interpolados | Português também no leitor de tela; evitar descrições da construção visual. |
| Fronteira Rust–UI | Mapeamentos de erro, motivos por arquivo e janelas nativas | Separar apresentação de diagnóstico; não traduzir indiscriminadamente todos os erros internos. |

Não houve execução de cada estado de tela, uso real com usuários ou simulação de todas as falhas. O plano fornece uma cobertura ampla de código e um catálogo comentado; a validação visual e funcional será parte da aplicação. Mensagens próprias do Windows, nomes de arquivos e conteúdo escrito pelo usuário não pertencem ao catálogo editorial do aplicativo.

O relatório visual, o extrator e o inventário completo desta revisão estão em `C:/Users/Usuario/AppData/Local/Temp/myalbuns-copy-audit-20260918/`. Este documento e a pesquisa permanecem no repositório.

## Constatações editoriais com evidência

As entradas abaixo são recomendações de escrita admitidas por seus consumidores de produção. Não recebem P0–P3: a revisão não produziu reprodução funcional de todos os estados de falha. A ordem expressa prioridade editorial e impacto na decisão da pessoa.

### 1. Salvar e recuperar ainda expõem termos de controle interno

- Admission route: Production-reachable.
- Reachability or current consumers: Salvar e Salvar como passam pelos adaptadores de persistência; seus mapas convertem códigos nativos em mensagens. Recuperação tem diálogo próprio antes da abertura do editor.
- Evidence: `src/platform/projectSaveFailure.ts:13–32`, `src/platform/projectSaveAsFailure.ts:14–39`, `src/components/ProjectRecoveryDialog.tsx:43–84`.
- Current impact: “revisão visível”, “Identidade física”, “arquivo regular” e “trabalho recuperável” exigem conhecer o modelo interno. “Atualize o Projeto” não corresponde a um comando nomeado assim no catálogo atual.
- Owning module: Adaptadores de persistência e apresentador de recuperação.
- Recommended change: Descrever alterações, arquivo e destino; manter a consequência de descartar. Antes de fechar as mensagens de estado indeterminado e revisão desatualizada, comprovar a ação segura disponível.
- Verification boundary: Salvar/Salvar como e recuperar pela janela real, incluindo conflito externo, cópia, limpeza incompleta e impossibilidade de determinar o resultado. Não provocar perda de alterações para testar uma frase.

### 2. O mesmo trabalho recebe nomes diferentes entre telas

- Admission route: Production-reachable.
- Reachability or current consumers: Personalização inicial, painel de propriedades, catálogo de comandos, recuperação de imagens na exportação e geração em lote.
- Evidence: `src/global/PersonalizationStep.tsx:142–268`, `src/application/projectCommandCatalog.ts:263`, `src/project-dialog/ProjectDialogView.tsx:69`, `src/generation/GenerationWindow.tsx:60–68`.
- Current impact: Frame, Background e Overlay se misturam ao português; Religar e Relinkar nomeiam a localização de imagens; Sobrescrever e Substituir nomeiam decisões equivalentes em fluxos diferentes.
- Owning module: Vocabulário de apresentação, catálogo de comandos e apresentadores de cada fluxo.
- Recommended change: Adotar o glossário abaixo de forma coordenada em rótulos, dicas, instruções e nomes acessíveis. Não fazer substituição global em identificadores ou arquivos de projeto.
- Verification boundary: Comparar o mesmo conceito na criação e no editor, nos menus e nos diálogos. Verificar localização por pasta, sem prometer busca em subpastas onde ela não existe.

### 3. Algumas instruções descrevem o mecanismo em vez da ação

- Admission route: Production-reachable.
- Reachability or current consumers: Tela de problemas da exportação → ação `openExportProject` → `dismissFeedback`; estados vazios dos painéis; criação de projeto.
- Evidence: `src/project-dialog/ProjectDialogView.tsx:63–78`, `src/components/ExportPreviewControl.tsx:138–142`, `src/components/MediaPanelEmptyState.tsx:16–35`, `src/global/PersonalizationStep.tsx:272`.
- Current impact: “Abrir Projeto” volta ao álbum já aberto; a orientação sobre “Lâmina selecionada” é estreita para a exportação de álbum ou intervalo; textos de estado vazio descrevem o futuro sem apontar a ação para começar.
- Owning module: Apresentador da exportação, estados vazios e etapa final da criação.
- Recommended change: “Voltar ao álbum”; “Resolva os problemas abaixo para exportar”; instruções com Importar, Novo projeto ou Abrir projeto onde esses controles já existem.
- Verification boundary: Confirmar exportação de lâmina, intervalo e álbum; verificar a volta ao editor sem afirmar que a lâmina da linha foi selecionada automaticamente.

### 4. O diagnóstico gráfico aparece na explicação principal

- Admission route: Production-reachable.
- Reachability or current consumers: Inicialização sem aceleração confirmada → `GlobalShell` → `SafeApplicationShell`; falha gráfica no projeto → `ProjectDialogView`.
- Evidence: `src/components/SafeApplicationShell.tsx:113–129`, `src/platform/graphics.ts:41–105`, `src/project-dialog/ProjectDialogView.tsx:188–197`.
- Current impact: WebGL2, backend, rasterização, Sessão e Canvas tornam uma impossibilidade de abrir o editor difícil de entender.
- Owning module: Diagnóstico gráfico e seus apresentadores global e do projeto.
- Recommended change: Explicar que o editor não pôde iniciar e manter detalhes de suporte acessíveis. Cada motivo real continua distinguível; não atribuir toda falha a driver ou afirmar que habilitar uma opção resolverá.
- Verification boundary: Contexto indisponível, aceleração por software e verificação inconclusiva. Preservar a exigência do ADR 0005 e o acesso às configurações.

### 5. Os textos de acessibilidade também precisam de revisão

- Admission route: Production-reachable.
- Reachability or current consumers: Prévia compartilhada pela criação e personalização do editor; campos e seletores de cor/escopo.
- Evidence: `src/ui/visualPreview/PersonalizationPreview.tsx:81,148,162,225`, `src/ui/ColorPropertyControl.tsx:60`, `src/global/PersonalizationStep.tsx:293`.
- Current impact: Rótulos como “Base branca canônica” e modelos que interpolam `side` descrevem implementação ou podem misturar idiomas. A presença de aria-label no código não prova que cada forma SVG seja anunciada em todos os leitores.
- Owning module: Controles e prévia compartilhados.
- Recommended change: Nomear objeto e ação em português; traduzir valores dinâmicos. Conferir se elementos apenas decorativos devem participar da árvore acessível antes de criar mais descrições.
- Verification boundary: Navegação por teclado e inspeção da árvore acessível; leitura do controle em contexto, sem duplicação excessiva.

## Oportunidade de organização das mensagens

Há uma fronteira de apresentação a completar, além da edição de frases.

- Admission route: Architectural leverage.
- Reachability or current consumers: `useProjectMutations`, `useLayoutCatalog`, `useMediaRemoval`, `useAlbumInformationApplyController` e `ExportPreviewControl` convertem falhas em texto com `Error.message` ou `String(error)`. O resultado chega a `useProjectOperationResultDialog` e ao `ProjectDialogView`. Importação mostra diretamente `ImageProcessingProblem.reason`.
- Evidence: `src/components/useProjectMutations.ts:54,120`, `src/components/useProjectOperationResultDialog.ts:29`, `src/project-dialog/ProjectDialogView.tsx:168`; `src-tauri/src/project_host.rs` mantém conversões `error.to_string()`; `src-tauri/src/photo_import.rs:873` inclui o motivo interno na mensagem de miniatura.
- Current impact: Texto destinado a diagnóstico pode determinar a redação pública. Trocar apenas os rótulos JSX deixa esses canais sem uma política editorial consistente. A revisão do canal não prova que todo erro interno tenha uma reprodução suportada.
- Owning module: Cada adaptador de operação e seu apresentador; o Core continua proprietário dos fatos e das regras.
- Recommended change: Usar os códigos estruturados já existentes, como nos mapas de persistência e Photoshop. Onde uma operação ainda retorna somente texto, propor um resultado com categoria e fatos necessários para a apresentação. Traduzir no proprietário do fluxo e manter o diagnóstico separado. Não criar substituições por palavras ou um tradutor global de exceções.
- Verification boundary: Uma falha representativa por operação na fronteira pública; preservar categoria, nome de arquivo, contagens, resultado parcial e ação possível. Detalhes técnicos permanecem disponíveis para diagnóstico, sem exigir que o usuário os entenda.

A aplicação deve ser incremental: catálogo de comandos continua responsável por comandos; controles compartilhados por rótulos e validações comuns; apresentadores por decisões e mensagens; Rust fornece fatos e resultados. Um novo sistema de internacionalização não é pré-requisito para esta revisão. Contratos tipados só devem ser ampliados para canais efetivamente usados e demonstrados.

## Decisões preservadas e itens excluídos

- [CONTEXT.md](../../CONTEXT.md) fixa hoje Frame, Background e Overlay e distingue Lâmina de Página. Primeiro registrar a equivalência dos nomes de apresentação propostos; não renomear o modelo interno nem o formato persistido por consequência de uma tradução.
- [Controles do editor](0040-fatos-do-core-e-controles-do-editor.md) determina tooltip para erros de campos, sem parágrafos de validação que mudem a altura do painel. A simplificação deve aproveitar esse padrão e conferir foco/teclado e associação do erro ao campo.
- [Progresso de operações](0007-progresso-de-operacoes.md) já exclui texto genérico de espera que surge e aumenta diálogos de decisão. Não reintroduzir “Concluindo…” em parágrafo, nem substituir a descrição da decisão por “Aguarde”. Indicadores de progresso reais, resultados parciais e estados que pedem uma ação continuam necessários.
- [Configurações](0009-configuracoes-do-aplicativo.md) fixa as abas Desempenho/Outros e a limpeza imediata ou agendada. A revisão de Cache não autoriza renomear essas abas ou alterar o momento da limpeza; os rótulos normativos devem ser atualizados junto da decisão editorial.
- [ADR 0005](../adr/0005-adotar-tauri-react-rust.md) mantém WebGL2 acelerado como requisito; escrever “editor” não muda esse requisito. Detalhes gráficos continuam úteis ao suporte.
- [ADR 0003](../adr/0003-limpar-saidas-orfas-pela-nomeacao.md) e o contrato de exportação tornam substituição e remoção de saídas anteriores consequências reais. Não reduzir todas as confirmações a um “Continuar?” genérico.
- Os textos “gate”, “shell” e “implementações fictícias” encontrados no fallback local de Configurações de `SafeApplicationShell` não são tratados como defeito comprovado da tela real. `src/global/main.tsx` fornece o callback da janela de Configurações, passado por `GlobalShell`; o fallback depende da ausência desse callback. Reavaliar a exposição se essa composição mudar.
- A pesquisa em issues não encontrou registros com o label `wontfix`; a busca por `wontfix` no corpo retornou #96, #98, #102, #104, #107 e #122, sobre persistência, progresso e ciclo de vida. Nenhum foi usado como justificativa para reabrir decisões de comportamento. `.out-of-scope/ui-acceptance-artifact-hashing.md` trata de evidência visual, sem relação com a revisão de redação.

## Limites da revisão e condições resolvidas

1. Nem todo `CoreError` convertido com `to_string()` pode ser produzido por uma interação suportada. Erros de invariantes, IDs inválidos, schema e resposta de protocolo permanecem candidatos internos até haver caminho público demonstrado. Não criar controles de recuperação para estados protegidos apenas porque existe uma mensagem.
2. “Fluxo de mudança dimensional segura” e “fluxo completo de conversão”, em `projectConfigurationFields.ts`, precisam ser confrontados com as guardas da configuração de álbum. Não foram classificados como falhas visíveis confirmadas.
3. A orientação para `stale_revision`, `save_state_indeterminate` e falha ao verificar identidade foi fechada conforme a tabela de resolução, preservando os estados de sessão definidos pelo Core.
4. A presença dos nomes acessíveis no código não comprova a leitura de cada elemento SVG. Revisar a árvore acessível antes de decidir entre renomear ou ocultar uma forma decorativa.
5. Não há medição de compreensão com usuários. Uma rodada curta de tarefas após a implementação pode confirmar se Quadro, Sobreposição e Prévias temporárias são reconhecidos sem explicação adicional.

## Vocabulário aceito

| Hoje | Proposta para a interface | Regra de uso |
| --- | --- | --- |
| Frame / placeholder | Quadro / quadro vazio | Não usar Moldura: Borda já representa outro atributo. |
| Background | Fundo | Mesma palavra na criação, no editor, em dicas e menus. |
| Overlay | Sobreposição | Não confundir com Fundo nem com Opacidade. |
| Canvas | Área de edição; editor em mensagens de falha | Escolha contextual, sem busca e substituição indiscriminada. |
| Religar / Relinkar | Localizar imagem… / Localizar imagens… | A ajuda explica a escolha da pasta e o alcance da busca. |
| Cache | Prévias temporárias | “Cache” pode continuar nos detalhes de suporte; não chamar de limpeza de fotos. |
| Sobrescrever | Substituir | Explicitar quais arquivos, sem omitir consequências. |
| Revisão visível / Sessão | Alterações atuais / projeto aberto | O erro determina a frase; não converter os termos mecanicamente. |
| Identidade física / arquivo regular | Explicação sobre o arquivo ou local escolhido | Detalhes do mecanismo ficam no diagnóstico. |
| Escopo | Aplicar em / lado da lâmina | Preservar o objeto e as opções existentes. |
| Pan | Posição da foto / arrastar a foto | Adequar à propriedade ou ao gesto, sem alterar atalhos. |
| DPI | Resolução (DPI) | DPI não é sinônimo de qualidade; manter unidade e intervalo real. |
| Lâmina / Página / Sangria | Manter | São distinções do trabalho; ajuda curta apenas no ponto necessário. |
| Layout | Manter nesta proposta | Palavra consolidada no domínio do programa; “disposição” serve na explicação do que o layout guarda. |
| Opacidade | Manter | “Transparência” inverteria a interpretação da escala. |
| JPEG / PNG / PDF / Photoshop | Manter | Formatos e nome do programa são informações de escolha. |

Usar maiúscula no início de rótulos e em nomes próprios: “Salvar projeto”, “Informações do álbum”, “Abrir no Photoshop”. A documentação pode continuar destacando conceitos; a capitalização da linguagem de domínio não precisa ser reproduzida em toda frase da interface.

## Guia curto de escrita

1. **Começar pela tarefa.** “Salvar alterações”, “Localizar imagem”, “Escolher pasta”. Evitar “Executar operação”, “Fluxo” e “Processar intenção”.
2. **Um conceito, um nome.** A instrução usa o mesmo nome do botão. Singular e plural acompanham a seleção real.
3. **Erro: situação e ação conhecida.** “A foto não foi encontrada. Escolha a pasta onde ela está.” Só usar quando a busca por pasta existir naquele contexto. Não oferecer “Tentar novamente” para uma falha sem caminho de repetição.
4. **Consequência antes da confirmação.** Dizer o que será descartado, removido ou substituído; distinguir remoção do projeto de exclusão no disco, quadros travados de destravados e resultado parcial de falha total.
5. **Ajuda no ponto da dúvida.** Primeiro melhorar o rótulo; depois usar a dica existente se ainda necessária. Não acrescentar um tutorial em cada painel.
6. **Detalhe técnico sob demanda.** Preservar diagnóstico, códigos e caminhos úteis ao suporte sem inseri-los na primeira frase. Não afirmar que uma nova área de detalhes já existe em fluxos que hoje não a oferecem.
7. **Não retirar orientação necessária.** Sangria, DPI, limites e formato de cor podem exigir precisão. Não inventar limites no frontend para obter uma frase mais amigável.
8. **Texto estável durante uma decisão.** Ocupação pode ser indicada pelos controles já previstos; não inserir parágrafos genéricos que ampliem a janela.
9. **Pontuação e tom.** Frases curtas, voz ativa, sem culpa, excesso de exclamações ou “por favor” automático. Botões sem ponto final; mensagens completas com ponto. Reticências indicam uma próxima escolha quando isso já faz parte do padrão do comando.
10. **Acessibilidade junto do rótulo.** Revisar texto visível, tooltip, nome acessível e valores interpolados na mesma alteração. Nenhuma informação essencial deve depender apenas de passar o mouse.

Esses critérios aplicam as fontes da pesquisa ao produto; não são uma declaração de conformidade com WCAG nem uma obrigação de copiar a aparência de outros sistemas.

## Etapas de aplicação

| Etapa | Trabalho | Donos principais | Critério de conclusão |
| --- | --- | --- | --- |
| 1. Vocabulário | Registrar nomes de apresentação, equivalências e capitalização; atualizar documentos afetados. | CONTEXT.md, desenhos de UI e catálogo de comandos | Uma tabela aprovada; nenhuma mudança em IDs ou formato de projeto exigida apenas pela tradução. |
| 2. Decisões e orientação | Salvar, Salvar como, recuperar, cópia, fechar, remover, conflitos e problemas da exportação. | Adaptadores de persistência e diálogos | Consequência correta e próxima ação executável em cada estado. Itens condicionais do catálogo resolvidos. |
| 3. Uso cotidiano | Menus, propriedades, criação, estados vazios, dicas e validações. | Catálogo de comandos, controles compartilhados e painéis | Mesmo conceito recebe o mesmo nome; limites continuam vindo do proprietário existente. |
| 4. Mensagens nativas | Abertura, gráficos, importação, Photoshop, exportação e geração; separar texto de suporte nos canais comprovados. | Adaptadores Tauri e proprietários das operações em Rust | Nenhum motivo técnico bruto conhecido chega sem apresentação adequada aos fluxos revisados; testes exercitam o contrato, não só a frase. |
| 5. Conferência contextual | Rótulos acessíveis, nomes longos, plural, escalas suportadas, teclado e estados de operação. | Superfícies e cenários de aceitação correspondentes | Legibilidade, comandos corretos e estabilidade dos diálogos; pesquisa final de termos antigos com justificativas para exceções. |

Aplicar por famílias completas de fluxo. Por exemplo, Localizar imagem deve mudar no menu, no diálogo de exportação e na mensagem do Photoshop antes de considerar a família concluída. Traduzir Frame deve incluir a criação, os layouts, o editor, as confirmações e as descrições acessíveis. Não dividir essas alterações em substituições isoladas que deixem a pessoa aprendendo dois nomes.

## Critérios de aceite da implementação

- Todo texto novo tem situação, superfície, proprietário e ação correspondente identificados.
- Formulários mantêm valores preenchidos e exibem a correção no tooltip existente; limites e unidades são reais.
- Botões não prometem abrir, selecionar, recuperar ou retomar quando apenas fecham um diálogo.
- Salvar, recuperar e salvar uma cópia permanecem distintos. Não chamar recuperação de salvamento automático.
- Confirmações de perda e substituição mantêm escopo, destino e consequências; não prometer Desfazer onde ele não existe.
- Resultados de importação/exportação distinguem concluído, ignorado, cancelado, falha e conclusão parcial quando essa informação muda a ação da pessoa.
- Frases dinâmicas funcionam com zero, um e vários itens, nomes de arquivo longos, caminhos de rede e caracteres acentuados.
- Rótulos e nomes acessíveis são revisados juntos; controles de ícone possuem nomes compreensíveis isoladamente.
- Não surgem novos parágrafos de espera ou erro em fluxo que desloquem os controles; conferir as dimensões e escalas declaradas pelos cenários existentes.
- Testes de contrato existentes são atualizados quando a apresentação muda. Novos testes são reservados para decisões, consequências e recuperação que precisem de prova, evitando uma bateria que apenas repita cada literal.
- A última busca por termos antigos admite justificativas: diagnóstico, identificador interno, formato de arquivo, documentação histórica ou termo profissional preservado. Zero ocorrências textuais não é um objetivo válido por si só.

## Catálogo de propostas

67 entradas selecionadas a partir do código atual. “Texto atual” reproduz o literal ou fragmento extraído; expressões entre chaves representam valores dinâmicos. São propostas de redação, não 67 defeitos confirmados. As entradas condicionais exigem validar o comportamento antes de fixar a mensagem. As entradas “Manter” registram exemplos já adequados.

### Salvar e recuperar

| ID / origem | Texto atual | Proposta | Motivo e condição |
| --- | --- | --- | --- |
| T01 · [src/components/ProjectRecoveryDialog.tsx:78](../../src/components/ProjectRecoveryDialog.tsx#L78) | O MyAlbuns encontrou trabalho concluído depois da última versão salva deste Projeto. | **Há alterações não salvas deste projeto. Deseja recuperá-las?** | Trocar a expressão abstrata “trabalho concluído” pela situação que a pessoa reconhece. **Redação proposta.** |
| T02 · [src/components/ProjectRecoveryDialog.tsx:74](../../src/components/ProjectRecoveryDialog.tsx#L74) | Reabrir e recuperar | **Recuperar e abrir** | Descrever a sequência da decisão. **Redação proposta.** |
| T03 · [src/components/ProjectRecoveryDialog.tsx:43](../../src/components/ProjectRecoveryDialog.tsx#L43) | Descartar recuperação e abrir | **Descartar alterações e abrir** | Explicitar o que será perdido. **Redação proposta.** |
| T04 · [src/components/ProjectRecoveryDialog.tsx:47](../../src/components/ProjectRecoveryDialog.tsx#L47) | A última versão salva será aberta e o trabalho recuperável será removido definitivamente. | **As alterações não salvas serão descartadas definitivamente. O projeto abrirá na última versão salva.** | Manter a consequência e a confirmação adicional. **Redação proposta.** |
| T05 · [src/components/ExternalCopyDecisionDialog.tsx:43](../../src/components/ExternalCopyDecisionDialog.tsx#L43) | Este arquivo é uma Cópia externa somente leitura. Escolha outro local para criar uma versão editável sem alterar o original. | **Para editar este arquivo, salve uma cópia em outro local. O original será mantido.** | Descrever a ação, mantendo o comando Salvar cópia como… e a independência da cópia. **Redação proposta.** |
| T06 · [src/platform/projectSaveFailure.ts:13](../../src/platform/projectSaveFailure.ts#L13) | A revisão visível ficou desatualizada. Atualize o Projeto e tente salvar novamente. | **O projeto mudou antes de o salvamento terminar.** | A segunda frase depende da recuperação real da projeção: não recomendar “Atualize o projeto”, pois não há comando com esse nome. Também não recomendar reabrir e perder alterações. **Depende de validar o fluxo.** |
| T07 · [src/platform/projectSaveAsFailure.ts:22](../../src/platform/projectSaveAsFailure.ts#L22) | Não foi possível comprovar a Identidade física do Projeto ou do destino. | **Não foi possível verificar o arquivo do projeto ou o destino escolhido.** | Retirar o mecanismo físico da frase principal. Definir o próximo passo por causa e ação disponível, sem prometer que repetir resolverá. **Depende de validar o fluxo.** |
| T08 · [src/platform/projectSaveAsFailure.ts:28](../../src/platform/projectSaveAsFailure.ts#L28) | O diálogo nativo de Salvar como não pôde ser aberto. | **Não foi possível abrir a janela para salvar o projeto.** | “Nativo” descreve a implementação. **Redação proposta.** |
| T09 · [src/platform/projectSaveFailure.ts:17](../../src/platform/projectSaveFailure.ts#L17) | Não foi possível confirmar qual revisão ficou no arquivo. Reabra o Projeto antes de continuar. | **Não foi possível confirmar a versão salva no arquivo.** | Preservar a incerteza; validar como reabrir com segurança antes de definir a orientação seguinte. Não afirmar “nada foi salvo”. **Depende de validar o fluxo.** |
| T10 · [src/platform/projectSaveFailure.ts:19](../../src/platform/projectSaveFailure.ts#L19) | O arquivo do Projeto foi salvo, mas não foi possível encerrar a Recuperação. Tente salvar novamente. | **O projeto foi salvo, mas a limpeza dos dados de recuperação não terminou. Tente salvar novamente.** | Preservar o sucesso do salvamento e separar a falha de limpeza; confirmar que a repetição continua disponível. **Depende de validar o fluxo.** |
| T11 · [src/platform/projectSaveFailure.ts:30](../../src/platform/projectSaveFailure.ts#L30) | O destino do Projeto deixou de ser um arquivo regular. | **Não é possível salvar no local atual.** | A solução pode exigir outro destino; conferir a disponibilidade de Salvar como antes de indicá-la. **Depende de validar o fluxo.** |
| T12 · [src/platform/projectSaveAsFailure.ts:20](../../src/platform/projectSaveAsFailure.ts#L20) | O Projeto escolhido como destino está aberto para edição em outra Sessão. | **O projeto escolhido está aberto em outra janela. Escolha outro arquivo para salvar a cópia.** | Evitar o conceito interno de Sessão; manter a exclusividade de edição. **Redação proposta.** |

### Editor e vocabulário

| ID / origem | Texto atual | Proposta | Motivo e condição |
| --- | --- | --- | --- |
| T13 · [src/application/projectCommandCatalog.ts:227](../../src/application/projectCommandCatalog.ts#L227) | Adicionar Frame | **Adicionar quadro** | Quadro é a proposta em português; não confundir com Borda. **Vocabulário proposto.** |
| T14 · [src/global/PersonalizationStep.tsx:142](../../src/global/PersonalizationStep.tsx#L142) | Background | **Fundo** | Aplicar também às dicas, menus, prévias e nomes acessíveis. **Vocabulário proposto.** |
| T15 · [src/global/PersonalizationStep.tsx:194](../../src/global/PersonalizationStep.tsx#L194) | Overlay | **Sobreposição** | Preservar a diferença entre fundo e imagem sobre o conteúdo. **Vocabulário proposto.** |
| T16 · [src/components/InspectorPanel.tsx:373](../../src/components/InspectorPanel.tsx#L373) | Frame placeholder | **Quadro vazio** | Nomear o estado visível sem introduzir outro objeto. **Vocabulário proposto.** |
| T17 · [src/application/projectCommandCatalog.ts:263](../../src/application/projectCommandCatalog.ts#L263) | Religar | **Localizar imagem…** | O fluxo pede uma pasta e procura o arquivo pelo nome; a dica deve dizer “Escolha a pasta onde está a imagem”. Não prometer pesquisa de subpastas. **Vocabulário proposto.** |
| T18 · [src/application/projectCommandCatalog.ts:270](../../src/application/projectCommandCatalog.ts#L270) | Substitui o vínculo da imagem contextual por outro arquivo. | **Troca a imagem selecionada por outro arquivo.** | A ação muda a imagem usada no projeto; não apaga o arquivo original. **Redação proposta.** |
| T19 · [src/application/projectCommandCatalog.ts:124](../../src/application/projectCommandCatalog.ts#L124) | Salva a revisão atual do Projeto. | **Salva as alterações do projeto.** | A pessoa salva alterações; revisão é um mecanismo de controle. **Redação proposta.** |
| T20 · [src/application/projectCommandCatalog.ts:171](../../src/application/projectCommandCatalog.ts#L171) | Desfaz a última alteração de domínio do Projeto. | **Desfaz a última alteração.** | Retirar o nome da camada do programa. **Redação proposta.** |
| T21 · [src/application/projectCommandCatalog.ts:180](../../src/application/projectCommandCatalog.ts#L180) | Refaz a última alteração de domínio desfeita. | **Refaz a alteração desfeita.** | Manter a ligação com Desfazer. **Redação proposta.** |
| T22 · [src/application/projectCommandCatalog.ts:153](../../src/application/projectCommandCatalog.ts#L153) | Abre o fluxo normal de Exportação do Projeto. | **Escolha o que exportar e onde salvar os arquivos.** | Explicar o próximo passo da ação. **Redação proposta.** |
| T23 · [src/application/projectCommandCatalog.ts:162](../../src/application/projectCommandCatalog.ts#L162) | Fecha a Janela do Projeto com proteção de alterações. | **Fecha o projeto e avisa se houver alterações não salvas.** | Dizer o que “proteção” representa neste fluxo. **Redação proposta.** |
| T24 · [src/components/SheetDesignInspector.tsx:211](../../src/components/SheetDesignInspector.tsx#L211) | Usando o design do álbum | **Usando o padrão do álbum** | Tornar a herança compreensível. **Vocabulário proposto.** |
| T25 · [src/components/SheetDesignInspector.tsx:211](../../src/components/SheetDesignInspector.tsx#L211) | Definido nesta lâmina | **Personalizado nesta lâmina** | Distinguir o ajuste local do padrão compartilhado. **Redação proposta.** |
| T26 · [src/components/SheetDesignInspector.tsx:233](../../src/components/SheetDesignInspector.tsx#L233) | Voltar ao design do álbum | **Usar padrão do álbum** | O verbo descreve a consequência para o valor. **Redação proposta.** |

### Começar e encontrar

| ID / origem | Texto atual | Proposta | Motivo e condição |
| --- | --- | --- | --- |
| T27 · [src/global/GlobalShell.tsx:288](../../src/global/GlobalShell.tsx#L288) | Os Projetos abertos recentemente aparecerão aqui. | **Crie um projeto ou abra um arquivo .myalbuns.** | Novo projeto e Abrir projeto já estão disponíveis nesta tela. **Redação proposta.** |
| T28 · [src/components/MediaPanelEmptyState.tsx:20](../../src/components/MediaPanelEmptyState.tsx#L20) | As Fotos importadas para este Projeto aparecerão aqui. | **Use Importar para adicionar fotos ao projeto.** | Indicar o controle existente que resolve o estado vazio. **Redação proposta.** |
| T29 · [src/components/MediaPanelEmptyState.tsx:16](../../src/components/MediaPanelEmptyState.tsx#L16) | As Imagens decorativas importadas aparecerão aqui. | **Use Importar para adicionar fundos e sobreposições.** | Relacionar o nome da aba ao uso dessas imagens. **Redação proposta.** |
| T30 · [src/components/MediaPanelEmptyState.tsx:26](../../src/components/MediaPanelEmptyState.tsx#L26) | Ajuste a Busca ou o Filtro de uso para ver outros itens. | **Tente outro nome ou altere o filtro.** | Oferecer duas saídas compreensíveis para uma busca sem resultados. **Redação proposta.** |
| T31 · [src/components/LayoutPanel.tsx:50](../../src/components/LayoutPanel.tsx#L50) | Escolha a quantidade de Frames para preparar um Layout. | **Escolha quantos quadros o layout deve ter.** | Retirar abstrações sem mudar a quantidade permitida. **Redação proposta.** |
| T32 · [src/components/LayoutPanel.tsx:84](../../src/components/LayoutPanel.tsx#L84) | Use o cadeado para aplicar e criar as posições adicionais. | **Use o cadeado para aplicar este layout com quadros vazios.** | Explicar o resultado do controle; conferir a variante de quantidade adicional na aceitação. **Redação proposta.** |
| T33 · [src/global/PersonalizationStep.tsx:272](../../src/global/PersonalizationStep.tsx#L272) | Nome e Localização serão escolhidos no diálogo do Windows ao criar. | **Na próxima etapa, escolha o nome e onde salvar o projeto.** | A etapa seguinte é o seletor de arquivo; não é necessário explicar quem implementa a janela. **Redação proposta.** |
| T34 · [src/global/NewProjectFlow.tsx:529](../../src/global/NewProjectFlow.tsx#L529) | Dimensão da Lâmina fechada | **Tamanho da lâmina fechada** | Preservar que a largura informada corresponde ao formato fechado. **Redação proposta.** |
| T35 · [src/global/NewProjectFlow.tsx:748](../../src/global/NewProjectFlow.tsx#L748) | Nenhuma | **Sem modelo** | Corrigir a concordância e explicitar a opção do seletor Modelo inicial. **Redação proposta.** |

### Campos e dicas

| ID / origem | Texto atual | Proposta | Motivo e condição |
| --- | --- | --- | --- |
| T36 · [src/application/projectConfigurationFields.ts:104](../../src/application/projectConfigurationFields.ts#L104) | Informe um DPI inteiro entre 1 e 1.200. | **Use um número inteiro entre 1 e 1.200 DPI.** | Manter intervalo, unidade e vínculo com o campo Resolução. **Redação proposta.** |
| T37 · [src/ui/ColorPropertyControl.tsx:25](../../src/ui/ColorPropertyControl.tsx#L25) | Use uma cor hexadecimal com seis dígitos, como #A1B2C3. | **Use uma cor no formato #A1B2C3.** | O exemplo comunica o formato sem exigir conhecer a palavra hexadecimal. **Redação proposta.** |
| T38 · [src/ui/ColorPropertyControl.tsx:60](../../src/ui/ColorPropertyControl.tsx#L60) | Cor hexadecimal ${label} | **Código da cor ${label}** | Simplificar também o nome acessível do campo. **Redação proposta.** |
| T39 · [src/application/projectConfigurationFields.ts:120](../../src/application/projectConfigurationFields.ts#L120) | A Sangria deve manter uma Área de corte positiva. | **Reduza a sangria para manter uma área de corte.** | Apresentar a correção conhecida. Um limite numérico só deve ser mostrado se vier dos fatos do Core. **Redação proposta.** |
| T40 · [src/application/projectConfigurationFields.ts:133](../../src/application/projectConfigurationFields.ts#L133) | Sangria e segurança devem manter uma Área de segurança positiva. | **Reduza a sangria ou a margem de segurança para manter uma área útil.** | Conferir a equivalência de “margem” no glossário antes de uniformizar o rótulo. **Vocabulário proposto.** |
| T41 · [src/components/PhotoZoomControl.tsx:17](../../src/components/PhotoZoomControl.tsx#L17) | Use ${props.minimum}% a ${props.maximum}%. | **Manter o padrão atual: Use {mínimo}% a {máximo}%.** | Já é curto, fornece a correção e usa os limites reais no tooltip. **Manter.** |
| T42 · [src/components/PhotoAngleControl.tsx:24](../../src/components/PhotoAngleControl.tsx#L24) | Use −45° a 45°, com uma casa decimal. | **Manter: Use −45° a 45°, com uma casa decimal.** | A restrição é útil; não remover a precisão necessária. **Manter.** |

### Exportar, remover e gerar

| ID / origem | Texto atual | Proposta | Motivo e condição |
| --- | --- | --- | --- |
| T43 · [src/project-dialog/ProjectDialogView.tsx:63](../../src/project-dialog/ProjectDialogView.tsx#L63) | Recupere os Arquivos necessários à Lâmina selecionada. | **Resolva os problemas abaixo para exportar.** | A exportação também aceita álbum e intervalo. As ações por arquivo continuam diferenciando ausente e indisponível. **Redação proposta.** |
| T44 · [src/project-dialog/ProjectDialogView.tsx:68](../../src/project-dialog/ProjectDialogView.tsx#L68) | Relinkar | **Localizar imagens…** | Usar o mesmo verbo do painel, com plural adequado ao fluxo de recuperação da exportação. **Vocabulário proposto.** |
| T45 · [src/project-dialog/ProjectDialogView.tsx:78](../../src/project-dialog/ProjectDialogView.tsx#L78) | Abrir Projeto | **Voltar ao álbum** | A ação openExportProject apenas dispensa o diálogo por dismissFeedback; não abre outro projeto nem seleciona automaticamente a linha. **Redação proposta.** |
| T46 · [src/project-dialog/ProjectDialogView.tsx:74](../../src/project-dialog/ProjectDialogView.tsx#L74) | Preencha os Frames vazios para exportar a seleção. | **Adicione fotos aos quadros vazios para exportar a seleção.** | Orientar pela tarefa e manter a indicação de lâmina e posição em cada linha. **Redação proposta.** |
| T47 · [src/project-dialog/ProjectDialogView.tsx:53](../../src/project-dialog/ProjectDialogView.tsx#L53) | O Layout será removido do catálogo em todas as Janelas. As composições aplicadas e as cópias guardadas nos Projetos serão preservadas. | **Este layout será removido da lista de personalizados. Os álbuns que já o usam e as cópias salvas nos projetos serão mantidos.** | Preservar o alcance global e a independência das composições já aplicadas. **Redação proposta.** |
| T48 · [src/project-dialog/ProjectDialogView.tsx:49](../../src/project-dialog/ProjectDialogView.tsx#L49) | Remover imagens e manter os Frames | **Remover imagens e manter quadros** | O texto alternativo “Remover tudo” também deve ser revisto junto da descrição dos quadros travados e destravados. **Redação proposta.** |
| T49 · [src/generation/GenerationWindow.tsx:110](../../src/generation/GenerationWindow.tsx#L110) | A hierarquia da origem será mantida. Escolha um destino fora da origem. | **A organização das pastas será mantida. Escolha um destino fora da pasta de origem e de suas subpastas.** | A restrição é relevante: “outro local” sozinho permitiria interpretar uma subpasta como destino válido. **Redação proposta.** |
| T50 · [src/generation/GenerationWindow.tsx:62](../../src/generation/GenerationWindow.tsx#L62) | Sobrescrever | **Substituir** | Uniformizar com a exportação; distinguir substituir arquivo existente de trocar imagem no álbum. **Vocabulário proposto.** |
| T51 · [src/generation/GenerationWindow.tsx:60](../../src/generation/GenerationWindow.tsx#L60) | Será sobrescrito | **Será substituído** | Manter coerência entre ação escolhida e resultado apresentado. **Redação proposta.** |
| T52 · [src/generation/GenerationWindow.tsx:49](../../src/generation/GenerationWindow.tsx#L49) | Continuar Geração | **Iniciar geração, quando os projetos ainda não começaram a ser gerados.** | Usar Continuar apenas para uma retomada real; verificar cada fase antes de mudar todas as ocorrências. **Depende de validar o fluxo.** |
| T53 · [src/batch-export/BatchExportWindow.tsx:21](../../src/batch-export/BatchExportWindow.tsx#L21) | O álbum atual foi publicado parcialmente. Libere espaço e retome para concluir. Os álbuns já exportados foram mantidos. | **Alguns arquivos do álbum atual já foram exportados. Libere espaço e retome para concluir. Os álbuns já exportados foram mantidos.** | “Publicado” é um detalhe da gravação. Preservar o resultado parcial e a ação Retomar. **Redação proposta.** |

### Configurações e integração

| ID / origem | Texto atual | Proposta | Motivo e condição |
| --- | --- | --- | --- |
| T54 · [src/settings/CacheSettings.tsx:68](../../src/settings/CacheSettings.tsx#L68) | Cache dos álbuns | **Prévias temporárias** | O nome explica o conteúdo; manter MB/KB e o alcance real da limpeza. **Vocabulário proposto.** |
| T55 · [src/settings/CacheSettings.tsx:77](../../src/settings/CacheSettings.tsx#L77) | Limpar cache | **Limpar prévias** | Deve ser a mesma ação em Configurações e no diálogo de falta de espaço. Não criar uma limpeza genérica de arquivos. **Vocabulário proposto.** |
| T56 · [src/settings/CacheSettings.tsx:81](../../src/settings/CacheSettings.tsx#L81) | Ao limpar o cache, as próximas aberturas dos álbuns podem demorar mais. | **As prévias serão recriadas quando necessário. Os álbuns podem demorar mais para abrir.** | A garantia sobre originais e projetos pode ficar na ajuda da ação; evitar sobrecarregar o balão. A limpeza agendada deve continuar explícita. **Redação proposta.** |
| T57 · [src/settings/CacheSettings.tsx:62](../../src/settings/CacheSettings.tsx#L62) | Limpeza agendada para a próxima inicialização. | **As prévias serão limpas quando você abrir o MyAlbuns novamente.** | Trocar “inicialização” pelo evento reconhecível. **Redação proposta.** |
| T58 · [src-tauri/src/photoshop.rs:39](../../src-tauri/src/photoshop.rs#L39) | O arquivo original da Foto está ausente. Religue a Foto antes de abri-la no Photoshop. | **A foto original não foi encontrada. Use Localizar imagem para indicar a pasta onde ela está.** | Só usar o novo nome depois de aplicá-lo ao menu correspondente. **Redação proposta.** |
| T59 · [src-tauri/src/photoshop.rs:54](../../src-tauri/src/photoshop.rs#L54) | Não foi possível abrir a seleção do executável do Photoshop. | **Não foi possível abrir a janela para localizar o Photoshop.** | Preservar Photoshop.exe como ajuda contextual no seletor, quando útil; retirar “executável” da falha. **Redação proposta.** |

### Inicialização e acessibilidade

| ID / origem | Texto atual | Proposta | Motivo e condição |
| --- | --- | --- | --- |
| T60 · [src/components/SafeApplicationShell.tsx:113](../../src/components/SafeApplicationShell.tsx#L113) | A criação e a abertura de Projetos permanecem bloqueadas porque este computador não confirmou a aceleração gráfica exigida pelo editor. | **Não foi possível iniciar o editor neste computador.** | O motivo simples e os detalhes de diagnóstico devem ser separados. Preservar acesso às configurações. **Redação proposta.** |
| T61 · [src/components/SafeApplicationShell.tsx:128](../../src/components/SafeApplicationShell.tsx#L128) | Somente o editor está bloqueado. Estas superfícies globais continuam funcionando sem iniciar uma Sessão do Projeto ou um Canvas. | **Você ainda pode acessar as configurações.** | Remover “superfícies globais”, Sessão e Canvas da explicação principal. **Redação proposta.** |
| T62 · [src/project-dialog/ProjectDialogView.tsx:197](../../src/project-dialog/ProjectDialogView.tsx#L197) | O Canvas não pôde ser iniciado | **Não foi possível iniciar o editor** | A pessoa reconhece a parte do produto afetada. **Redação proposta.** |
| T63 · [src/platform/graphics.ts:105](../../src/platform/graphics.ts#L105) | O WebGL2 disponível está usando rasterização por software, que não atende ao editor. | **O editor precisa de aceleração gráfica, que não está disponível nesta tentativa.** | Guardar WebGL2 e a causa precisa nos detalhes. Outras causas precisam de mensagens próprias; não atribuir tudo a um driver. **Redação proposta.** |
| T64 · [src/ui/visualPreview/PersonalizationPreview.tsx:81](../../src/ui/visualPreview/PersonalizationPreview.tsx#L81) | Base branca canônica | **Fundo branco** | “Canônica” é uma propriedade da implementação. **Redação proposta.** |
| T65 · [src/ui/visualPreview/PersonalizationPreview.tsx:148](../../src/ui/visualPreview/PersonalizationPreview.tsx#L148) | Frame demonstrativo ${side} ${frameNumber} | **Quadro de exemplo {número}, lado esquerdo/direito** | Traduzir o valor interpolado de side, além do texto fixo. Conferir se cada forma precisa ser anunciada pelo leitor de tela. **Redação proposta.** |
| T66 · [src/global/PersonalizationStep.tsx:293](../../src/global/PersonalizationStep.tsx#L293) | Escopo da personalização | **Aplicar personalização em** | Manter as opções Lado esquerdo, Ambos os lados e Lado direito conforme o contexto. **Redação proposta.** |
| T67 · [src/ui/visualPreview/SheetGuideLayer.tsx:22](../../src/ui/visualPreview/SheetGuideLayer.tsx#L22) | Guias técnicas da Lâmina | **Guias de corte e segurança da lâmina** | Nomear a finalidade; conferir que o grupo não inclui outro tipo de guia antes de fixar o rótulo. **Depende de validar o fluxo.** |

## Fontes primárias

- [Microsoft — escrita para Windows](https://learn.microsoft.com/en-us/windows/apps/design/style/writing-style)
- [Apple — escrita de interface](https://developer.apple.com/design/human-interface-guidelines/writing)
- [Apple — alertas](https://developer.apple.com/design/human-interface-guidelines/alerts)
- [GOV.UK — mensagens de erro](https://design-system.service.gov.uk/components/error-message/)
- [W3C — sugestões de correção](https://www.w3.org/WAI/WCAG22/Understanding/error-suggestion.html)
- [Gov.br — linguagem simples](https://www.gov.br/pt-br/guia-de-edicao-de-servicos-do-gov.br/escrevendo-o-seu-texto)

A pesquisa vinculada descreve o que cada fonte sustenta e o que é uma aplicação local ao MyAlbuns. Consulta em 18/09/2026.
