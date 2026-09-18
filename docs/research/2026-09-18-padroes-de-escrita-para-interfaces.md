---
status: current
document: research
date: 2026-09-18
---

# Padrões de escrita para interfaces

## Pergunta e alcance

Como simplificar os textos do MyAlbuns para que a pessoa entenda o que está
acontecendo e o que pode fazer em seguida, sem precisar conhecer a implementação?

Esta pesquisa reúne seis fontes primárias oficiais, consultadas em 18 de setembro
de 2026. As recomendações abaixo são uma aplicação editorial ao produto; não
alteram seu comportamento, não substituem as fontes normativas e não comprovam
conformidade de acessibilidade. Os guias de Microsoft, Apple e governos são
publicações contínuas, sem versão de biblioteca aplicável. A referência W3C é a
explicação informativa do critério 3.3.3 da WCAG 2.2.

## Evidências

### Microsoft: explicar a situação e oferecer uma saída possível

O guia de escrita para aplicativos Windows recomenda linguagem familiar, voz
ativa e informação principal antes dos detalhes. Em erros, orienta explicar o
problema sem culpar a pessoa, eliminar jargão e apresentar uma solução que ela
consiga executar. Os rótulos de botões devem comunicar a ação de forma breve. A
consequência para o MyAlbuns é revisar a mensagem junto com os comandos disponíveis
naquele estado, em vez de escrever uma orientação genérica para todo tipo de falha.
[Fonte: Writing style — Windows apps](https://learn.microsoft.com/en-us/windows/apps/design/style/writing-style).

### Apple: consistência entre palavras, contexto e próximo passo

A orientação de escrita da Apple recomenda manter uma lista de termos comuns,
usar rótulos com verbos e adaptar o tom à situação. Estados vazios devem indicar
uma ação útil, preferencialmente com acesso direto quando possível. Rótulos
descritivos também ajudam quem utiliza leitor de tela. Para o MyAlbuns, isso
sustenta um vocabulário editorial único e instruções que apontem para o nome real
de um controle, sem depender de frases como “clique aqui” ou de uma posição visual.
[Fonte: Writing — Human Interface Guidelines](https://developer.apple.com/design/human-interface-guidelines/writing).

### Apple: alertas devem tornar a decisão compreensível

O guia de alertas recomenda títulos específicos, texto adicional apenas quando
necessário e botões que descrevam o resultado de sua escolha. O tom deve ser
direto, sem esconder a gravidade da situação. Uma ação destrutiva precisa oferecer
uma saída segura para cancelar. No MyAlbuns, a aplicação editorial é nomear o
objeto afetado e a consequência real; os detalhes de aparência, ordem e acionamento
dos botões continuam sujeitos às decisões do produto Windows.
[Fonte: Alerts — Human Interface Guidelines](https://developer.apple.com/design/human-interface-guidelines/alerts).

### GOV.UK: distinguir erros de entrada de problemas do aplicativo

O Design System britânico orienta mensagens específicas, que retomem as palavras
do rótulo e expliquem a correção. Recomenda preservar os valores preenchidos e
evitar mensagens abstratas, como um simples aviso de valor inválido. Também
distingue erros corrigíveis no formulário de problemas do serviço. Essa distinção
ajuda a separar, no MyAlbuns, uma dimensão fora do intervalo permitido de uma
falha ao abrir ou salvar um arquivo: as duas situações não pedem a mesma resposta.
[Fonte: Error message — GOV.UK Design System](https://design-system.service.gov.uk/components/error-message/).

### W3C: informar como corrigir quando a correção é conhecida

A explicação do critério 3.3.3 da WCAG 2.2 estabelece que erros de entrada
detectados devem receber sugestões de correção conhecidas, salvo exceções ligadas
à segurança ou finalidade do conteúdo. A orientação destaca benefícios para
pessoas com limitações cognitivas, visuais e motoras. Para o MyAlbuns, a pesquisa
adota isso como critério editorial de validação: indicar o campo, a restrição e
uma correção comprovável. Não transforma toda falha interna em erro da pessoa.
[Fonte: Understanding Error Suggestion — W3C](https://www.w3.org/WAI/WCAG22/Understanding/error-suggestion.html).

### Gov.br: linguagem simples em português

O guia de edição do Gov.br recomenda frases curtas e diretas, verbos específicos,
palavras conhecidas e consistência para o mesmo conceito. Termos técnicos
indispensáveis podem permanecer, acompanhados de uma explicação adequada ao
público. A aplicação ao MyAlbuns é simplificar a tarefa descrita e retirar o
vocabulário de engenharia, sem apagar as distinções necessárias ao trabalho de
diagramação. Brevidade deve vir da remoção de informação dispensável, preservando
as condições que mudam a decisão da pessoa.
[Fonte: Escrevendo o seu texto — Gov.br](https://www.gov.br/pt-br/guia-de-edicao-de-servicos-do-gov.br/escrevendo-o-seu-texto).

## Critérios propostos para a revisão

As regras desta seção são inferências para o MyAlbuns a partir das evidências,
combinadas com seu domínio e a direção solicitada pelo usuário.

| Assunto | Critério editorial |
| --- | --- |
| Ação | Começar pelo verbo que expressa o resultado: “Abrir Projeto”, “Adicionar Fotos”, “Salvar como”. Usar o mesmo nome nas instruções e no controle correspondente. [Microsoft](https://learn.microsoft.com/en-us/windows/apps/design/style/writing-style). |
| Erro | Informar o que impediu a tarefa e o próximo passo disponível. “Tentar novamente” exige que repetir seja possível e pertinente. Não inventar causa, solução ou promessa de preservação dos dados. [Microsoft](https://learn.microsoft.com/en-us/windows/apps/design/style/writing-style). |
| Campo | Nomear o campo e sua restrição concreta, incluindo a unidade quando necessária. Evitar apenas “Valor inválido”. Limites numéricos devem vir do comportamento real. [GOV.UK](https://design-system.service.gov.uk/components/error-message/), [W3C](https://www.w3.org/WAI/WCAG22/Understanding/error-suggestion.html). |
| Consequência | Dizer o que será removido, substituído ou descartado e qual o alcance. Só afirmar que é possível desfazer quando isso estiver comprovado naquele fluxo. Preservar a opção de cancelar. [Apple Alerts](https://developer.apple.com/design/human-interface-guidelines/alerts). |
| Estado vazio | Explicar brevemente o que falta e indicar uma ação existente para começar. Não esconder informação permanente em um aviso que desaparece após o primeiro uso. [Apple Writing](https://developer.apple.com/design/human-interface-guidelines/writing). |
| Leitura e acessibilidade | Preferir nomes que façam sentido ao serem lidos isoladamente, incluindo os nomes acessíveis de controles com ícones. Erros devem permitir identificar a correção sem adivinhar qual campo foi afetado. [Apple Writing](https://developer.apple.com/design/human-interface-guidelines/writing), [W3C](https://www.w3.org/WAI/WCAG22/Understanding/error-suggestion.html). |

## Vocabulário do produto e termos técnicos

O usuário desta revisão prefere nomes em português. A direção proposta é
**Quadro**, **Fundo** e **Sobreposição** para os conceitos hoje chamados de Frame,
Background e Overlay. “Moldura” não é o nome proposto para Quadro, pois pode ser
confundido com sua Borda. Essa mudança precisa ser conciliada com
[CONTEXT.md](../../CONTEXT.md), que atualmente fixa os nomes em inglês e registra
restrições a alguns sinônimos. Esta pesquisa registra a direção, sem alterar o
glossário.

**Lâmina**, **Página**, **Sangria**, **Layout** e **Resolução (DPI)** conservam
distinções úteis. Lâmina e Página não são intercambiáveis; DPI não deve virar um
rótulo genérico como “qualidade”. A explicação de uma unidade ou termo profissional
deve esclarecer seu uso naquele campo. A preferência por palavras conhecidas não
exige eliminar conceitos necessários ao trabalho.
[Domínio do MyAlbuns](../../CONTEXT.md),
[orientação de linguagem simples](https://www.gov.br/pt-br/guia-de-edicao-de-servicos-do-gov.br/escrevendo-o-seu-texto).

Expressões de implementação, como `gate`, `shell`, identidade física, revisão
visível, processo interno ou serialização, não constituem uma orientação de uso.
A revisão deve recuperar a tarefa que essas expressões tentam explicar. Uma
proposta local: se houver colisão comprovada com o arquivo aberto e for possível
escolher outro nome, explicar essas duas informações. Esse exemplo depende da
confirmação do fluxo; não é uma mensagem pronta para toda falha de salvamento.

## Densidade e validação das propostas

A [referência visual vigente](../references/ui-programa-diagramacao/README.md)
continua sendo a base do produto. Simplificar os textos não exige acrescentar
parágrafos a todos os painéis. Para esta revisão, propõe-se primeiro melhorar o
rótulo; depois, se necessário, acrescentar uma única explicação no ponto da dúvida.

Uma instrução extensa pode revelar um problema no fluxo, além do problema de
redação. Essa leitura segue a orientação da Apple de reconsiderar interações que
a linguagem sozinha não consegue esclarecer.
[Apple Writing](https://developer.apple.com/design/human-interface-guidelines/writing).

Cada proposta deve ser conferida na superfície em que aparece: botões legíveis,
mensagens completas, nomes longos de arquivos, plural, estados de erro e escalas
de exibição suportadas. O objetivo é evitar que a revisão desloque controles,
esconda a ação principal ou dependa de cortes de texto para caber. Este é um
critério local de aceitação visual, não um limite universal de caracteres extraído
das fontes.

Antes de aprovar um texto, a revisão deve conseguir responder: qual situação o
aciona, o que a pessoa precisa entender, que ação existe naquele estado, que
consequência precisa conhecer e onde o texto será apresentado. A comprovação do
comportamento e da apresentação pertence à implementação futura; esta pesquisa
não declara que esses requisitos já foram atendidos.
