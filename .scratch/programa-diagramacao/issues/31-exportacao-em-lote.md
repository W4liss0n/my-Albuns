# 31 — Exportação em lote

**What to build:** localizar recursivamente Projetos persistidos e exportar serialmente o Álbum inteiro de cada um, em Modo de lote exclusivo, preservando a hierarquia de destino e isolando os resultados por item.

**Blocked by:** 13 — Bloqueio de abertura; 28 — PDF multipágina; 29 — Limpeza de saídas órfãs.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [configuração da Exportação em lote](../../../docs/design/0006-configuracao-da-exportacao-em-lote.md); [Tela de Problemas](../../../docs/design/0005-tela-de-problemas.md); [progresso de operações](../../../docs/design/0007-progresso-de-operacoes.md); [ADR 0006 — publicação com transação limitada](../../../docs/adr/0006-publicar-exportacao-com-transacao-limitada.md); [ADR 0007 — caminhos Windows e identidade física](../../../docs/adr/0007-tratar-caminhos-windows-e-identidade-fisica.md); [armazenamento local e Cache](../../../docs/design/0010-armazenamento-local-e-cache.md); [resolução e política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] O diálogo seleciona pasta de Projetos, Formato, Modo (`Por lâmina` ou `Por página`) e Destino; não oferece Intervalo nem Qualidade, pois cada item é sempre o Álbum inteiro e JPEG usa qualidade máxima.
- [ ] Origem e Destino aceitam os caminhos Windows totalmente qualificados do contrato; o `BatchRunner` reutiliza um `OperationPathContext` durante descoberta e planejamento e o congela em um `RootBindingPlan` usado por todos os processos participantes, sem persistência entre lotes ou retomadas.
- [ ] Descobrir Projetos recursivamente e trabalhar exclusivamente com a versão persistida em disco, mesmo quando uma Janela desse Projeto possuir mudanças não salvas.
- [ ] A Boas-vindas abre uma janela dedicada com origem, Formato, Modo, Destino, contagem encontrada e `Cancelar`/`Verificar e exportar`.
- [ ] Um `BatchRunner` externo ao pipeline descobre, abre pela interface do `ProjectCore`, migra e valida todos os documentos antes de adquirir o Modo de lote exclusivo.
- [ ] Mostrar pendências na Tela de Problemas com `Abrir Projeto`, `Relinkar` individual, `Relinkar todos`, `Tentar novamente` para recurso indisponível e `Ignorar neste lote`; nunca iniciar automaticamente depois das correções.
- [ ] Correções que modificam o Projeto, como preencher placeholder, só resolvem a pendência depois de `Salvar`; o lote descarta a sessão de validação e reabre a versão persistida.
- [ ] A Religação individual escolhe uma raiz para aquele Projeto; a global escolhe a raiz que contém pastas de arquivos dos Projetos e só pesquisa dentro de pasta cujo Nome corresponda exatamente ao Nome do Projeto.
- [ ] Ambas as religações procuram recursivamente por nome e extensão exatos e resolvem automaticamente somente uma correspondência única. Ausência ou ambiguidade continua como pendência.
- [ ] Manter religações individual e global em mapas temporários separados por item e válidos somente para a execução; não regravar Projeto, não criar Undo/Redo e descartar os mapas ao concluir ou cancelar.
- [ ] Exigir que cada item esteja corrigido ou explicitamente ignorado e que o usuário clique em `Continuar Exportação`; fechar antes disso cancela o lote.
- [ ] Ao continuar, adquirir `BatchExclusive` no `OperationGate`, obter a pausa do `CacheEngine`, bloquear interação em todas as Janelas de Projeto e manter interativa somente a janela global de progresso/cancelamento.
- [ ] Processar exatamente um Projeto por vez e manter no máximo um `MyAlbuns.Imaging.exe` temporário ativo no lote. Calibração, perfil de desempenho e paralelismo entre Álbuns ficam fora do MVP.
- [ ] Imediatamente antes de cada item, reabrir seu documento persistido com `ProjectCore`, registrar revisão/hash e repetir validação; qualquer mudança desde a pré-validação invalida o resultado anterior.
- [ ] Revalidar revisão/hash imediatamente antes de criar o `RenderSnapshot`; se o arquivo mudou novamente, não usar o snapshot antigo e devolver o item à validação ou marcá-lo como falha acionável.
- [ ] Para cada item, produzir um `RenderSnapshot` imutável e chamar o mesmo `ExportPipeline` da Exportação normal; o Processador recebe snapshot e plano de bindings e não interpreta diretamente o documento do Projeto.
- [ ] Abrir individualmente cada Projeto e original necessário mesmo quando sua raiz já foi resolvida; rede, servidor ou compartilhamento indisponível não confirma Arquivo ausente e afeta somente o item correspondente.
- [ ] Por padrão, publicar ao lado de cada arquivo de Projeto, dentro de uma pasta com o Nome do Projeto; em destino alternativo, preservar a hierarquia relativa e criar essa pasta por Projeto.
- [ ] Pré-calcular conflitos do lote e exigir `Sobrescrever todos` ou `Cancelar`; JPEG, PNG e PDF reutilizam composição, nomes, validações e originais da Exportação normal.
- [ ] Para cada item, renderizar e verificar em preparação reservada dentro da própria pasta de Destino e executar a transação limitada do ADR 0006; nunca anunciar sucesso antes da publicação completa.
- [ ] Publicar com substituição atômica por arquivo quando suportada, sem rollback do conjunto; se um item falhar após começar a promoção, avisar que seu Destino pode conter mistura e não remover nenhum órfão. Limpar órfãos JPEG/PNG somente depois do sucesso completo daquele item.
- [ ] Persistir atomicamente um checkpoint simples em `%LOCALAPPDATA%\MyAlbuns\Recovery\Batches\{batch-id}.json`, com opções e estado `pending`, `completed`, `ignored` ou `failed` por item, além do item corrente.
- [ ] Nunca retomar no meio de renderização ou publicação. Se o processo cair, tratar o item corrente como `pending`, descartar sua preparação incompleta e refazê-lo integralmente após confirmação explícita.
- [ ] Depois de reiniciar, oferecer `Retomar` ou `Encerrar`; `Retomar` captura um novo `RootBindingPlan`, pula itens concluídos/ignorados, refaz o interrompido e processa pendentes, enquanto `Encerrar` preserva saídas já publicadas e remove o checkpoint.
- [ ] Cancelamento durante a preparação remove a tentativa quando possível; depois que a publicação começa, respeitar o envelope limitado do ADR, nunca anunciar sucesso, marcar o item para ser refeito e permitir `Retomar` ou encerrar explicitamente.
- [ ] Projeto inválido, mídia ausente, recurso indisponível, alteração concorrente ou outra falha determinística afeta somente seu item e aparece no resultado; não existe repetição automática por múltiplos Processadores de Imagens.
- [ ] Durante a execução, mostrar somente progresso geral e `X/Y`, com `Cancelar` como única ação. Sucesso integral recebe confirmação curta; ignorados ou falhas abrem a Tela de Problemas.
- [ ] Ao concluir, falhar, cancelar ou encerrar, liberar concessão, pausa, Janelas e recursos da tentativa; nenhum Projeto é salvo ou alterado automaticamente.
- [ ] Testes cobrem descoberta recursiva, destino padrão/alternativo, mudanças não salvas ignoradas, correção salva, religações temporárias estritas e únicas, alteração de revisão/hash, conflito global, serialização, cancelamento, queda antes/durante/depois da publicação e retomada que refaz o item interrompido.
