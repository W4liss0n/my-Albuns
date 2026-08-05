---
status: historical
document: technical-research
ticket: fase-2-fluxo-persistente-01
date: 2026-08-03
updated: 2026-08-05
---

# Reconciliar a fronteira de caminhos da próxima fase

Type: task

Status: resolved

Blocked by: None

## Question

Quais critérios ainda abertos de `Política e resolução de caminhos Windows` são pré-requisitos reais do documento, da criação e da Exportação desta fase; quais pertencem à integração posterior; e como as dependências do tracker devem representar essa divisão sem marcar trabalho inexistente como concluído?

A resposta deve inventariar os oito critérios abertos, preservar explicitamente `MyAlbuns2` durante esta fase e eliminar o bloqueio artificial causado pela migração final do namespace.

## Answer

A fronteira anterior misturava fundação, integração de produto e migração final. A reconciliação encontrou o seguinte para os oito critérios que estavam abertos:

1. **`AppPaths` e nome da raiz:** descoberta das Known Folders, categorias e raízes centralizadas já estão implementadas. Somente o nome final não está: `MyAlbuns2` é intencional. A fundação ficou encerrada no registro histórico `Política e resolução de caminhos Windows`, enquanto a transição final foi isolada em [Migração do namespace temporário de dados](https://github.com/W4liss0n/my-Albuns/issues/40).
2. **Criação segura de alvo novo:** existem primitivas físicas para Cache e preparação da Exportação, mas ainda não existe o fluxo autoritativo de criação do arquivo de Projeto. Esse trabalho é requisito do primeiro fluxo e passou para [Integração de caminhos no primeiro fluxo real](https://github.com/W4liss0n/my-Albuns/issues/13).
3. **Resultados tipados:** já existem resultados especializados para resolução, ausência, indisponibilidade, tipo inesperado, acesso negado, raiz não vinculada, evidência física inconclusiva, conflito de trava e falha de I/O. O critério estava com o checkbox desatualizado e foi encerrado sem criar uma enumeração genérica duplicada.
4. **Filhos seguros e reparse points:** Cache e Exportação já mantêm diretórios por handle, validam filhos físicos e rejeitam redirecionamentos. A parte ausente é aplicar a mesma garantia ao arquivo de Projeto novo; ela foi movida junto da criação segura para `Integração de caminhos no primeiro fluxo real`.
5. **Preparação e Publicação:** a preparação já fica dentro do Destino, possui ciclo de descarte e promoção única e preserva a transação limitada. A implementação corrente cobre a fundação; o consumo pelo JPEG real permanece nos tickets de Renderização e integração.
6. **Cache por chave opaca:** `project-{sha256}`, armazenamento local, publicação segura e contenção física já estão implementados sob a raiz temporária de `AppPaths`. Somente a troca do nome da raiz foi reservada à migração final.
7. **Host, diálogo e interface reais:** o runtime ainda abre o Projeto demo, portanto este critério não está implementado. Ele pertence a `Integração de caminhos no primeiro fluxo real`, depois dos contratos de Projeto, Renderização e UI e antes do esqueleto ponta a ponta.
8. **Matriz Windows:** a fundação corrente cobre formas aceitas e proibidas, UNC, unidade mapeada, aliases, caminhos verbatim e longos, round-trip nativo, identidade física, Cache, staging e reparse points. A matriz restante — especialmente alvo novo, acesso negado real e o fluxo público — passou para o ticket de integração.

O grafo resultante mantém `Política e resolução de caminhos Windows` como fundação anterior aos designs de Projeto e Renderização. `Integração de caminhos no primeiro fluxo real` depende desses designs e da UI, e passou a bloquear `Esqueleto ponta a ponta`. `Migração do namespace temporário de dados` depende das folhas terminais da primeira versão e não bloqueia nenhuma fase intermediária.

Validação corrente: `cargo test -p myalbuns-paths` concluiu 35 testes, sem falhas; os dois testes de gate real que exigem o runner Windows/UNC permaneceram corretamente ignorados no comando comum.
