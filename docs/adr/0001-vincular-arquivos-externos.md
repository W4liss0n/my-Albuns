---
status: accepted
date: 2026-07-27
---

# Vincular arquivos de mídia em vez de incorporá-los

Os Projetos armazenam os caminhos nativos dos arquivos de mídia originais, e suas cópias duplicam apenas essas referências. Os vínculos podem apontar para discos locais, UNC, unidades mapeadas ou caminhos verbatim locais/UNC. A decisão evita duplicar arquivos grandes e mantém os Projetos leves, aceitando que arquivos movidos ou excluídos precisem ser localizados novamente.

## Consequências

- Alterar, excluir ou relocalizar uma referência dentro de um Projeto nunca modifica outro Projeto.
- Se uma origem acessível confirmar que o arquivo externo não existe no caminho registrado, todos os Projetos que ainda apontam para esse caminho passam a considerá-lo ausente.
- Se o conteúdo de um arquivo for substituído no mesmo caminho, a nova versão é o original autoritativo para todos os Projetos vinculados e para a Exportação.
- Projetos abertos monitoram seus caminhos vinculados. Eventos rápidos são consolidados e tratados como indícios; quando o arquivo estiver estável e legível, uma inspeção autoritativa confirma o estado. Só então o estado observado e o Cache são atualizados. Ausência confirmada sob uma origem acessível marca o vínculo como ausente, e o retorno ao caminho registrado o recupera sem Religação. Nada disso altera a referência, cria Undo/Redo ou marca o Projeto.
- Ao reabrir um Projeto, a validação comum compara tamanho e data de alteração, sem recalcular o hash completo de todos os originais. A primeira versão aceita o risco raro de uma mudança externa conservar ambos exatamente e manter temporariamente uma representação interativa antiga; a Exportação continua reabrindo o original atual e nunca usa essa representação como fonte.
- Cada Projeto corrige suas referências de forma independente, salvo quando o arquivo retorna ao caminho originalmente registrado.
- Falha de rede ou de acesso que impeça confirmar a existência produz Arquivo indisponível, não Arquivo ausente, e nunca reescreve automaticamente o vínculo.
- A forma textual do caminho não é usada como identidade física; representações diferentes do mesmo alvo continuam sendo resolvidas pela política do [ADR 0007](0007-tratar-caminhos-windows-e-identidade-fisica.md).
- O Projeto não é autocontido, e uma representação em cache nunca pode substituir o arquivo original durante a exportação.
