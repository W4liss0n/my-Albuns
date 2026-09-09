---
status: accepted
document: design
date: 2026-09-08
ticket: 22
---

# Ângulo fino da Foto e Projeto v5

Este recorte entrega o Ângulo definido no
[design da Janela do Projeto](0001-estrutura-da-janela-do-projeto.md), após
[Giro e Espelhamento](0021-orientacao-de-fotos-e-projeto-v4.md). O Ângulo altera
a Foto dentro do Frame, com prévia no Canvas, Histórico, Salvamento, reabertura
e Exportação. A geometria do Frame, o vínculo do arquivo original e os demais
ajustes são preservados.

## Controle e Histórico

O Painel contextual apresenta o Ângulo entre Giro e Espelhamento. O slider e
o campo numérico usam o intervalo inclusivo de `−45°` a `45°`, em passos de
`0,1°`. Valores positivos giram a Foto no sentido anti-horário. Giro e Ângulo
são independentes, incluindo suas restaurações.

O campo aceita ponto ou vírgula decimal. Enter ou saída do campo confirma uma
entrada válida. Valores fora do intervalo, incompletos ou com precisão maior
que uma casa decimal não são gravados; ao sair, o valor anterior reaparece.
Escape descarta o rascunho. As setas para cima e para baixo ajustam o campo
em um décimo de grau.

Durante um arraste, cada amostra consulta a composição no Core sem editar o
Projeto. Soltar confirma uma única ação. Uma sequência de repetição de tecla
no slider também confirma uma vez, ao soltar a tecla. Cancelamento de ponteiro,
perda inesperada da captura, bloqueio da interface ou saída da janela descartam
o rascunho. Uma mudança externa da seleção, sem conclusão prévia do controle,
também descarta o rascunho restante. A liberação normal da captura depois de
soltar o ponteiro não cancela a edição.

Dois cliques no slider retornam a zero como uma única ação de Undo/Redo, sem
botão adicional. Para distinguir o primeiro clique de um clique duplo, a
confirmação de um clique sem arraste aguarda o intervalo configurado no Windows.
A aplicação lê `GetDoubleClickTime`, sem alterar a preferência. Um arraste real
usa o limiar de movimento do Windows e confirma ao soltar. Sair do controle ou
acionar outro comando confirma antes de enfileirar a próxima operação. Clicar
outro Frame conclui a edição antes de mudar a seleção; essa confirmação tem
precedência sobre a invalidação do contexto anterior.

Na seleção múltipla, o valor comum é apresentado; divergências mostram `—`,
sem posicionar visualmente o indicador do slider em um valor inventado. A
primeira edição define um valor absoluto para todas as Fotos selecionadas em
uma ação. Placeholders são preservados e continuam contados na indicação do
escopo. Uma seleção contendo somente placeholders não exibe o controle.

## Fronteira do Core e prévia

`SetPhotoAngle { edit: PhotoAngleEdit }` recebe IDs de Frames e `angleTenths`,
um inteiro entre `−450` e `450`. O Core valida uma seleção não vazia, sem IDs
repetidos, pertencente a uma única Lâmina. Seleções inválidas ou valores fora do
intervalo falham antes de alterar o Projeto. Aplicar o valor já existente não
consome revisão nem Histórico.

`preview_photo_angle` passa pela mesma edição do documento e pelo mesmo
projetor usados na confirmação. Retorna os `ComposedFrame` selecionados, sem
alterar a sessão, o Histórico ou os arquivos. O frontend substitui apenas esses
Frames na apresentação temporária; não recalcula rotação, preenchimento ou Pan.
Respostas atrasadas são descartadas. Uma mudança da composição autoritativa
solicita novamente a prévia do rascunho atual.

Confirmações compartilham a fila de mutações com Giro, Espelhamento, Salvar e
Undo/Redo. Capturam os IDs selecionados, preservando o alvo se a navegação mudar
enquanto aguardam. Uma falha pendente cancela os comandos dependentes. Salvar
consulta a revisão autoritativa quando chega sua vez na fila.

## Composição e Pan com Espelhamento

O Ângulo soma-se ao Giro antes do Espelhamento horizontal. O plano do renderizador
usa coordenadas com Y para baixo; por isso seu valor de rotação em graus é
`90 × quarterTurns − angleTenths / 10`. A interface conserva o sinal positivo
anti-horário do ajuste do usuário.

Para que toda a área do Frame permaneça preenchida nos limites de Pan, os eixos
de deslocamento acompanham a inclinação efetiva da Foto. Quando há Espelhamento,
somente o sinal da parcela fina desses eixos é invertido. Em coordenadas do
plano, o ângulo dos eixos é `90 × quarterTurns + angleTenths / 10` com
Espelhamento, e coincide com a rotação do plano sem Espelhamento. Isso mantém
exatamente o sentido dos controles nos projetos v4, cujo Ângulo é zero.

Esse refinamento também se aplica à formulação futura Q32.32 do
[contrato do renderizador final](0019-contrato-do-renderizador-final.md): os
cossenos e senos usados no centro deslocado da Foto são os dos eixos de Pan;
os usados na rotação da imagem e no preenchimento continuam os da rotação total.
O pipeline atual continua consumindo o plano de composição comum ao Canvas,
à miniatura e ao processador de Exportação.

## Arquivo v5

A versão 5 acrescenta `angleTenths` obrigatório ao objeto `photo.transform`.
Mantém os demais campos da versão 4. A cadeia de migração tipada
`v1 → v2 → v3 → v4 → v5` acrescenta Ângulo zero ao passar de v4 para v5 e
preserva Giro, Espelhamento, Pan e Zoom.

Abrir uma versão anterior migra somente em memória, sem alterar os bytes, a
revisão salva ou o Histórico. Um Salvamento explícito grava v5. Cada versão
mantém seu DTO fechado: v4 continua rejeitando `angleTenths`; v5 rejeita campo
ausente, tipo não inteiro, valor fora do intervalo e campos desconhecidos.
A reidentificação de uma cópia mantém a versão de origem até o Salvamento.

## Evidência de comportamento

Os testes públicos do Core cobrem prévia sem efeitos, confirmação única,
seleções mistas, extremos, validação atômica, Undo/Redo, migração e reabertura.
Um teste independente inverte a transformação dos quatro cantos do Frame para
verificar cobertura com Pan extremo, dois níveis de Zoom, todos os Giros,
Espelhamento e Fotos horizontais ou verticais. O processador real exporta
quadrantes conhecidos para conferir o sentido do Ângulo e a ordem com Giro e
Espelhamento, preservando os bytes da Foto original.

O corpus visual deriva dessas fronteiras públicas. Os cenários de aceitação
cobrem prévia, entrada numérica, limites, valor inválido, seleção múltipla,
restauração por dois cliques, teclado, Histórico e escalas de 100%, 125% e 150%.
Os contratos externos consultados foram os de entradas controladas e eventos
do React 19.2, captura de ponteiro do navegador e
[GetDoubleClickTime](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getdoubleclicktime).
