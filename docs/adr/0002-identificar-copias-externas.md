---
status: accepted
date: 2026-07-27
updated: 2026-08-10
---

# Atribuir nova identidade a cópias externas

Cada Projeto possui uma identidade interna independente de nome e localização. `Salvar como` gera uma nova identidade desde o início; quando um arquivo fisicamente diferente criado pelo sistema operacional repete a Identidade de outro Projeto e a origem continua acessível, o programa atribui automaticamente uma nova Identidade à duplicata. Um alias local, UNC ou mapeado do mesmo arquivo físico apenas focaliza sua sessão. Isso permite abrir original e cópia simultaneamente sem confundir duas representações do mesmo alvo.

Para que essa distinção continue possível depois do fechamento, um registro local por Identidade conserva a última Localização autorizada fora do arquivo de Projeto. A Localização serve somente para reencontrar a instância anterior e obter nova evidência física por handles; o pathname não se torna Identidade nem prova sozinho `Same` ou `Different`. O registro sobrevive à Sessão e à limpeza de Cache, enquanto a identidade física viva e o Bloqueio de abertura continuam pertencendo à tentativa ou à Sessão. O contrato detalhado está em [Contrato público de persistência do ProjectCore](../design/0015-contrato-publico-de-persistencia-do-project-core.md).

## Consequências

- Renomear ou mover um Projeto preserva sua identidade.
- Uma duplicata externa recebe nova identidade na primeira detecção, sem manter relação com o original.
- Corrigir a identidade pode gravar metadados automaticamente, mas nunca persiste alterações de diagramação não salvas.
- Se a Cópia externa não puder receber sua nova identidade — por exemplo, por estar em mídia somente leitura — sua sessão editável não é aberta e nenhum Cache ou Recuperação é montado sob a identidade duplicada. A interface oferece `Salvar cópia como...` para criar um arquivo gravável com nova Identidade ou permite cancelar.
- Se uma origem acessível confirmar que o caminho anterior não existe mais, o novo caminho é tratado como movimentação, não como cópia. Indisponibilidade ou acesso negado não autoriza essa inferência.
- Caminhos locais, UNC, unidades mapeadas e formas verbatim locais ou UNC podem ser aliases do mesmo arquivo. Quando o alvo existe, comparação física e bloqueio real impedem classificar um alias como Cópia externa ou abrir duas sessões editáveis.
- Se a comparação física for inconclusiva, o aplicativo falha de forma fechada diante de conflito de bloqueio em vez de presumir que os caminhos representam Projetos diferentes.
- A ausência legítima de registro local representa a primeira observação daquela Identidade nesta máquina; um registro existente que não possa ser lido ou cuja Localização não possa ser inspecionada não é tratado como ausência e falha de forma fechada.
- A pasta de Cache usa a identidade, não Nome ou caminho: movimentações reutilizam o Cache, enquanto `Salvar como` e Cópias externas usam namespaces independentes.
- A identidade de uma Cópia externa é resolvida antes de montar qualquer estado local por Projeto, inclusive Cache e Recuperação, impedindo que ela acesse temporariamente o namespace do original.
