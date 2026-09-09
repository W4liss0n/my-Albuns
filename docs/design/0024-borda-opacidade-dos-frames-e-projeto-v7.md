---
status: accepted
document: design
date: 2026-09-09
ticket: 22
---

# Borda e Opacidade dos Frames e Projeto v7

Este recorte entrega Borda e Opacidade no [Painel contextual](0001-estrutura-da-janela-do-projeto.md),
seguindo o [contrato do renderizador](0019-contrato-do-renderizador-final.md).
As propriedades pertencem ao Frame, inclusive quando está vazio. Copiar Frames
preserva seu estilo; trocar o conteúdo mantém o estilo no Frame de destino.

## Estilo herdado e próprio

Frames novos e migrados usam a Borda atual do álbum e Opacidade de 100%.
Qualquer alteração manual em cor, espessura ou Opacidade torna o estilo inteiro
próprio, conservando os outros valores resolvidos naquele momento. Alterar o
design do álbum passa a atingir somente os Frames que continuam herdando.
`Voltar ao design do álbum` recupera a Borda vigente, restaura 100% de Opacidade
e retoma a herança. Esta alteração é uma única ação de Histórico para toda a seleção.

Espessura zero representa ausência de Borda. O estilo próprio conserva sua cor
mesmo com espessura zero, permitindo escolher uma cor antes de aumentar a Borda
ou remover e recolocar a Borda sem perder a cor escolhida.

## Controles e Histórico

`Design` contém Opacidade inteira entre 0 e 100%, espessura na unidade física
de apresentação e um seletor de cor RGB. O campo numérico aceita qualquer
espessura canônica válida; o slider oferece de zero a 5 mm e se amplia para
conter uma espessura existente maior. Dois cliques restauram 100% de Opacidade
ou espessura zero. O intervalo dos cliques e a distância de arraste vêm do Windows.

Na seleção mista, campos numéricos apresentam `—` e a amostra de cor fica vazia.
A primeira entrada explícita aplica o valor absoluto aos Frames selecionados,
incluindo placeholders, sem uniformizar as outras propriedades. O seletor de
cor permite prévia, cancelamento e aplicação explícita, inclusive quando a cor
escolhida coincide com a cor inicial do seletor em uma seleção mista.

A prévia é resolvida pelo Core, sem revisão, sujeira ou Histórico. Confirmar
uma propriedade, Salvar ou desfazer passa pela fila autoritativa de mutações;
uma propriedade pendente é confirmada antes do comando seguinte. Cancelamento
ou troca de seleção invalida respostas atrasadas. Os IDs da seleção são capturados
ao iniciar o comando, e falhas cancelam os comandos dependentes.

## Composição

Cada `ComposedFrame` recebe sua Borda resolvida, os retângulos do anel interno
sem sobreposição e `opacityByte`. A Borda reduz a área visível da Foto sem
alterar o tamanho do Frame ou seu enquadramento. O limite do recuo é calculado
separadamente para cada eixo. Uma borda física positiva pode ocupar zero pixels
na resolução de saída; não há espessura mínima artificial de um pixel.

Foto e Borda formam um grupo transparente. A Opacidade incide uma vez sobre
esse grupo, depois dos ajustes da Foto e antes da composição sobre os elementos
inferiores. O contrato inteiro é:

```text
opacityByte = floor((opacityPercent * 255 + 50) / 100)
effectiveAlpha = floor((sampleAlpha * opacityByte + 127) / 255)
```

O Canvas usa `AlphaFilter` no grupo de conteúdo e libera o filtro junto ao nó,
preservando os programas compartilhados do PixiJS. Contornos de seleção,
alças e instruções do placeholder permanecem fora do grupo. Um Frame com
Opacidade zero continua selecionável e ocupa a mesma posição na pilha.
A prévia SVG usa Opacidade no grupo equivalente. O Processador escolhe Foto
ou Borda por pixel e aplica o alfa uma vez, incluindo cantos e bordas saturadas.
O amostrador atual continua em uso; o compositor completo Q32.32 permanece
vinculado ao Programa 04.

## Persistência e compatibilidade

O arquivo v7 acrescenta `style` obrigatório a cada Frame: `album`, ou `custom`
com `border { rgb, widthUm }` e `opacityPercent`. A cadeia tipada v1–v7 mantém
os DTOs anteriores fechados; v6 rejeita `style` e v7 rejeita campos ausentes,
desconhecidos e valores inválidos. Migrar v6 acrescenta `album` sem alterar a Foto.
Abrir migra em memória; somente Salvamento explícito grava v7. Reidentificar
uma cópia conserva o esquema de origem até esse Salvamento.

A IPC do Processador passa de 20 para 21, pois `ComposedFrame` ganha campos
obrigatórios. Processadores anteriores não podem ignorar silenciosamente o estilo.

## Verificação

As fronteiras públicas cobrem herança, estilo próprio, restauração, seleção
mista, validação atômica, ausência de alterações redundantes, Histórico, cópia,
troca de conteúdo, Salvamento e reabertura. O Processador real compara cores
conhecidas em Opacidade 0, 50 e 100%, Foto inferior e cantos da Borda.
O corpus do Core alimenta a aceitação visual no Canvas, na prévia SVG e na Grade,
incluindo placeholders, valores mistos, cancelamento e escalas de 100%, 125% e 150%.

O contrato externo consultado é o de [filtros do PixiJS 8](https://pixijs.com/8.x/guides/components/filters),
na versão instalada 8.19.0, junto da documentação instalada de `AlphaFilter`
para Opacidade aplicada ao grupo após renderizar seus conteúdos.
