---
status: current
document: research
date: 2026-09-05
updated: 2026-09-05
---

# Estado do produto e próxima entrega

**O MyAlbuns já possui um primeiro fluxo produtivo implementado, mas ainda não é o MVP completo. A próxima entrega recomendada é importar várias Fotos JPEG em uma única seleção.**

Esta revisão retrata o commit `3be5ce5d0fcce20a099d95f75937ff3d7c8a772d`, na [PR #64](https://github.com/W4liss0n/my-Albuns/pull/64), ainda aberta. O código dessa branch não deve ser confundido com uma versão já integrada ou liberada. Foram consultados o tracker atual, os caminhos produtivos, os contratos e os resultados existentes. Não foram executados novos testes, abertas janelas ou preparados ambientes.

A revisão orienta a ordem de trabalho. Não altera a especificação, não encerra tickets e não aprova a liberação do produto.

## O que já temos

“Implementado” nesta tabela significa que existe caminho produtivo e evidência de testes registrada. A aceitação nativa atual continua limitada pelo fechamento descrito adiante.

| Área | Estado e utilidade atual | Evidência principal |
| --- | --- | --- |
| Criação e abertura | Global, criação com dimensões e personalização, arquivo `.myalbuns` e Host por Projeto. | [Jornada produtiva](0034-jornada-produtiva-ponta-a-ponta.md), [Global](../../src/global/GlobalShell.tsx), [criação](../../src/global/NewProjectFlow.tsx). |
| Persistência e Histórico | Salvamento explícito, Undo/Redo, reabertura, Identidade, Salvar como, bloqueio de abertura e Recuperação de sessão. | Tickets [#10](https://github.com/W4liss0n/my-Albuns/issues/10), [#14](https://github.com/W4liss0n/my-Albuns/issues/14), [#15](https://github.com/W4liss0n/my-Albuns/issues/15), [#18](https://github.com/W4liss0n/my-Albuns/issues/18); testes públicos em `crates/myalbuns-core/tests/`. |
| Estrutura do Álbum | Lâminas, extremidades, adição, exclusão e reordenação; conversão de extremidade no recorte já suportado. A conversão completa com reorganização futura não está toda entregue. | [Álbum físico #9](https://github.com/W4liss0n/my-Albuns/issues/9), [intenções atuais](../../crates/myalbuns-core/src/model.rs), [testes](../../crates/myalbuns-core/tests/physical_album_structure.rs). |
| Primeira composição | Importar uma Foto JPEG, reutilizar vínculo existente, adicionar/substituir a Foto por duplo clique ou soltura, composição com Frame e Pan/Zoom da Foto. | [Primeira composição](0037-primeira-composicao-com-foto.md), [importação real](../../src-tauri/src/project_commands.rs), [testes](../../crates/myalbuns-core/tests/photo_composition_v3.rs). |
| Painel de imagens | Abas, miniaturas, busca, filtros e seleção já possuem implementação. A importação atual oferece um arquivo JPEG por vez; o painel completo ainda tem ações futuras. | [Painel](../../src/components/MediaPanel.tsx), [barra de importação](../../src/components/MediaPanelToolbar.tsx). |
| Mídias e Cache | Vínculos externos, prévias reconstruíveis, observação de mudanças, distinção entre ausência e indisponibilidade, Religação e recuperação do Processador. | [Integração de Mídias e Cache](0036-integracao-final-de-midias-e-cache.md), ticket [#45](https://github.com/W4liss0n/my-Albuns/issues/45). |
| Exportação inicial | JPEG de uma Lâmina, usando a revisão visível e os Originais. | [Exportação produtiva](../../src-tauri/src/export_commands.rs), [contrato do primeiro JPEG](../design/0014-contrato-jpeg-do-primeiro-fluxo.md). |
| Base de validação | Validação de build, contratos, tipos, interface, automação e Rust sem janelas. | [CI aprovada do commit revisado](https://github.com/W4liss0n/my-Albuns/actions/runs/33940289150). |

O recibo histórico da [jornada de 29 de agosto](artifacts/0023-productive-journey.json) pertence ao commit `3ac5fbb`, com WebView2 151. Ele registra passos nativos aprovados, mas não retém os artefatos brutos daquela rodada. Serve como evidência histórica, não como aprovação nativa do commit atual ou do WebView2 152.

Há também [evidência histórica de instalação em máquina limpa](0026-distribuicao-windows-e-pendencia-de-maquina-limpa.md). O nome antigo desse arquivo não significa que aquela instalação permaneça pendente. Uma versão candidata futura precisará de sua própria validação.

## O que ainda falta

Os tickets fechados [#2](https://github.com/W4liss0n/my-Albuns/issues/2), [#3](https://github.com/W4liss0n/my-Albuns/issues/3) e [#4](https://github.com/W4liss0n/my-Albuns/issues/4) são de **design**. Por exemplo, fechar o contrato do Renderizador final não entrega automaticamente PNG, PDF e Exportação completa.

| Frente | Lacuna observável no código ou escopo ainda aberto | Ticket proprietário |
| --- | --- | --- |
| Importação cotidiana | Vários arquivos, pasta, soltura do sistema operacional, demais formatos e resultado parcial. O seletor e o resolvedor atuais restringem a importação de Foto a JPEG individual. | [#19](https://github.com/W4liss0n/my-Albuns/issues/19). |
| Edição geométrica | Mover/redimensionar grupos, oito alças, seleção múltipla de Frames, ordenação da pilha e criação explícita de placeholder. `ProjectIntent` ainda não oferece as mutações geométricas completas. | [#20](https://github.com/W4liss0n/my-Albuns/issues/20). |
| Design por Lâmina | Background/Overlay locais, herança e retorno ao padrão. Há estrutura visual e padrões do Álbum; controles locais ainda são marcados como futuros. | [#21](https://github.com/W4liss0n/my-Albuns/issues/21), [inspector local](../../src/components/SheetDesignInspector.tsx). |
| Estilos e transformações | Efeitos, giro, espelhamento, ângulo e edição coletiva completos. Pan/Zoom existente não encerra esse escopo. | [#22](https://github.com/W4liss0n/my-Albuns/issues/22). |
| Layouts | Aplicação, travamento, personalizados e favoritos; gerador ainda depende de decisão. | [#25](https://github.com/W4liss0n/my-Albuns/issues/25)–[#29](https://github.com/W4liss0n/my-Albuns/issues/29). |
| Painel e reutilização completos | Organização e ações restantes do Painel, remoção de mídias, duplicação de Lâmina e reutilização de Frames. | [#24](https://github.com/W4liss0n/my-Albuns/issues/24), [#30](https://github.com/W4liss0n/my-Albuns/issues/30), [#33](https://github.com/W4liss0n/my-Albuns/issues/33), [#34](https://github.com/W4liss0n/my-Albuns/issues/34). |
| Exportação completa | Álbum/intervalo, modos por Lâmina/Página, PNG, PDF e tratamento completo de múltiplas saídas. O menu geral `Exportar…` ainda é futuro; `Exportar Lâmina…` está implementado. | [#35](https://github.com/W4liss0n/my-Albuns/issues/35), [#37](https://github.com/W4liss0n/my-Albuns/issues/37), [#38](https://github.com/W4liss0n/my-Albuns/issues/38); [menus](../../src/components/projectApplicationMenus.ts). |
| Operações em lote | Geração e Exportação em lote completas. | [#36](https://github.com/W4liss0n/my-Albuns/issues/36), [#39](https://github.com/W4liss0n/my-Albuns/issues/39). |
| Integrações e fechamento do MVP | Configurações globais completas, Photoshop, integração restante de caminhos, catálogo completo, transformação dimensional e migração final de dados. Alguns já têm partes implementadas. | [#12](https://github.com/W4liss0n/my-Albuns/issues/12), [#13](https://github.com/W4liss0n/my-Albuns/issues/13), [#16](https://github.com/W4liss0n/my-Albuns/issues/16), [#23](https://github.com/W4liss0n/my-Albuns/issues/23), [#5](https://github.com/W4liss0n/my-Albuns/issues/5), [#31](https://github.com/W4liss0n/my-Albuns/issues/31), [#32](https://github.com/W4liss0n/my-Albuns/issues/32), [#40](https://github.com/W4liss0n/my-Albuns/issues/40). |

Essas lacunas pertencem às funcionalidades planejadas. Não foram promovidas a novos bugs. A revisão não reabre decisões de escopo: `MyAlbuns2` permanece durante o desenvolvimento conforme #40; remapeamento de atalhos e automações de Cache excluídas do MVP continuam excluídos. A proposta de hashing adicional de evidências visuais permanece [fora de escopo](../../.out-of-scope/ui-acceptance-artifact-hashing.md).

## O que bloqueia uma liberação agora

1. **Fechar Projeto continua sem validação nativa atual.** Existe relato e evidência de travamento após Salvar como e edições em original/cópia. As provas sem janelas não reproduziram o travamento. A [pesquisa de fechamento](2026-09-04-isolamento-fechamento-projeto.md) delimita o que está demonstrado. A PR #64 segue aberta; a CI aprovada não autoriza tratá-la como integralmente validada ou mesclá-la.
2. **O Sandbox atual falha antes de alcançar esse fechamento.** A [sondagem local de 5 de setembro](../../.tools/isolated-close-sandbox/20260905-graphics-early-1/RESULTADO.md) registrou a mesma NVIDIA na Global e no Host, falha na criação do contexto e quedas do processo gráfico do WebView2. Isso é um bloqueio do ensaio atual, cuja causa original não foi estabelecida; não é uma reprodução do travamento de fechamento.
3. **O escopo funcional do MVP ainda está incompleto.** Mesmo resolver a infraestrutura gráfica não entregaria as frentes da tabela anterior.

A falta de uma prova nativa bloqueia a liberação que depende dela. Não impede a implementação e verificação de funcionalidades independentes. Não serão abertas novas rodadas do Sandbox como consequência automática desta revisão.

## Próxima entrega recomendada: importar várias Fotos JPEG

**Resultado para o usuário:** escolher vinte Fotos em uma única seleção, vê-las no Painel e poder desfazer a entrada do conjunto de uma vez.

Este é um primeiro recorte de [#19 — Ciclo de mídias externas](https://github.com/W4liss0n/my-Albuns/issues/19). Seu bloqueador #17 está fechado. Mantém os formatos do caminho produtivo atual para entregar valor sem incluir, na mesma rodada, novos decodificadores, regras de pasta, soltura do sistema operacional ou edição geométrica.

| Alternativa | Avaliação para a próxima entrega |
| --- | --- |
| Importação múltipla JPEG | Remove a repetição do diálogo por Foto e reutiliza Painel, Cache, composição e JPEG existentes. É o recorte recomendado. |
| Edição completa de Frames | Tem alto valor e bloqueadores #17/#9 concluídos, mas reúne seleção, limites de geometria, alças, grupos e pilha. Deve ser fatiada depois, não assumida como uma pequena alteração. |
| Exportação completa ou Layouts | Há dependências funcionais ainda abertas. Antecipar tudo ampliaria a rodada. |
| Outra investigação de VM/GPU | Não entrega função do produto; permanece suspensa até uma necessidade concreta de aceitação. |

### Critérios de conclusão desse recorte

- Na aba Fotos, uma única ação `Importar` permite selecionar vários arquivos `.jpg`/`.jpeg`; cancelar o seletor não altera o Projeto.
- As Fotos válidas aparecem no Painel com prévias pelo Cache e ficam disponíveis para a composição existente.
- Repetições na seleção e vínculos já existentes não criam duplicatas nem são apresentados como falha. Mantém-se a regra de equivalência de caminho do produto, sem mesclar aliases físicos por iniciativa nova.
- Um arquivo inválido ou inacessível não perde os demais válidos. O resultado apresenta os rejeitados com Arquivo e Motivo, no padrão da Tela de Problemas. Não basta ignorar erros silenciosamente.
- Todas as novas referências da seleção formam uma única ação de Undo/Redo. Desfazer remove somente esses vínculos; Refazer restaura o conjunto. Uma seleção sem novidade não cria Histórico nem alterações pendentes.
- Salvar e reabrir preservam os novos vínculos; nenhum byte do Original é incorporado ao Projeto ou alterado. Uma Foto importada nessa seleção pode ser colocada em Frame e exportada pelo JPEG existente, usando o Original.
- Durante a inspeção, a interface representa a operação pendente e impede conflito com mutações concorrentes; leituras de arquivos ficam fora da thread da interface. Uma tentativa usa seu contexto de caminhos proprietário.

Concluir esse recorte **não encerra #19**. PNG/TIFF, Decorativos, pasta, soltura externa e Religação ampliada permanecem no ticket proprietário. O MVP continua exigindo esses formatos e fluxos; JPEG individual é apenas o ponto de partida atual.

### Implementação e prova esperadas

O caminho atual é `MediaPanelToolbar` → `ProjectWorkspace`/controlador → `tauriProjectPorts.importPhoto` → comando Tauri `import_photo` → `MediaResolver` → `EditableProject.import_photo` → `ProjectSession`. O Host conserva os caminhos nativos; a WebView recebe resultados e identificadores.

A mudança deve manter esses proprietários e acrescentar a unidade de importação da seleção no núcleo. Repetir `import_photo` em um laço não atende ao aceite: [a implementação atual](../../crates/myalbuns-core/src/persistent_session.rs) cria uma entrada no Histórico para cada chamada.

A verificação principal pode ocorrer sem janelas: arquivos temporários reais na fronteira do resolvedor/núcleo; lote misto com duplicatas e falhas; Undo/Redo único; persistência/reabertura; concorrência no controlador; e projeção do resultado no componente público. O JPEG deve ser verificado pela fronteira produtiva que já existe. Capturas visuais, quando houver mudança de superfície, usam os cenários headless pertinentes e a revisão de referência exigida pelo repositório.

A integração específica do seletor nativo exige uma verificação curta de seleção múltipla e cancelamento, em momento combinado. Enquanto ela não acontecer, o estado será “implementado e validado sem janelas; seletor nativo pendente”. Não haverá aprovação nativa inferida nem retomada automática da jornada inteira de fechamento.

## Forma de continuar

A próxima rodada deve ter somente a entrega acima como objetivo, em uma mudança própria, sem ampliar a PR #64. O planejamento da base de código deve preservar as dependências existentes e o bloqueio de integração dessa PR; sua pendência não será apagada para permitir a nova entrega.

Depois desse recorte, avançar na edição de Frames em entregas separadas e concluir as demais partes de mídias conforme as dependências. O mapa completo continua no [GitHub #1](https://github.com/W4liss0n/my-Albuns/issues/1); esta revisão não cria um segundo tracker.

## Verificação desta revisão

A CI do commit revisado está aprovada em `Headless validation`; o piloto nativo está `SKIPPED`. Os dados do tracker foram consultados em 5 de setembro e a cópia local fica em `.tools/delivery-review-20260905/issues.json`. Os bloqueadores também foram lidos nos corpos dos tickets, pois a listagem de relações nativas veio vazia. Nenhum ticket, comentário ou PR foi alterado no GitHub.

Nesta rodada foram produzidos somente este retrato de entrega e a correção dos apontadores do README. A revisão não constitui auditoria exaustiva de código, nova aceitação visual, execução de testes ou implementação da importação múltipla.