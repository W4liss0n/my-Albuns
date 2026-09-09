---
status: accepted
document: design
date: 2026-09-08
ticket: 22
---

# Preto e branco da Foto e Projeto v6

Este recorte entrega o efeito definido no [Painel contextual](0001-estrutura-da-janela-do-projeto.md)
depois de [Giro e Espelhamento](0021-orientacao-de-fotos-e-projeto-v4.md) e
[Ângulo fino](0022-angulo-fino-da-foto-e-projeto-v5.md). Preto e branco pertence à
ocorrência da Foto: duas colocações da mesma mídia podem ter efeitos diferentes.
O arquivo original, o Cache compartilhado e a miniatura no Painel de imagens
preservam suas cores.

## Controle, seleção e Histórico

`Ajustes e Efeitos` é uma seção recolhível própria, abaixo de `Design`. Contém
somente `Preto e branco`, como botão de alternância com rótulo estável e estado
acessível ligado, desligado ou misto. Não aparece quando a seleção só contém
placeholders. Na seleção múltipla, informa quantas Fotos serão atingidas entre
os Frames selecionados.

`TogglePhotoBlackAndWhite { frame_ids }` recebe uma seleção não vazia de Frames
distintos da mesma Lâmina. O Core calcula o próximo valor a partir do estado
autoritativo: se todas as Fotos selecionadas já têm o efeito, desativa; caso
contrário, ativa em todas. Placeholders são preservados. A seleção inválida
falha antes de qualquer alteração; selecionar somente placeholders não consome
revisão nem Histórico.

Cada alteração forma uma ação de Undo/Redo. O comando compartilha a fila de
mutações com os outros ajustes, Salvar e Undo/Redo e confirma um rascunho de
Ângulo antes de continuar. Captura os IDs selecionados; mudar a seleção durante
a espera não redireciona a alteração. Uma falha cancela comandos dependentes.

Pan, Zoom, Giro, Ângulo e Espelhamento conservam o efeito. Copiar ou trocar o
conteúdo leva o efeito com a Foto. Substituir a Foto por uma nova colocação
começa com o efeito desligado. Desativá-lo recupera as cores, preservando os
demais ajustes.

## Composição e renderização

O Core projeta `blackAndWhite` no estado da Foto e em `ComposedPhoto`. Canvas,
prévia da Lâmina, grade de miniaturas e Exportação consomem esse mesmo plano.
O efeito alcança os pixels da Foto após a amostragem, antes da Borda. Não altera
Background, Overlay, Borda ou outras Fotos.

A luminância segue o [contrato do renderizador](0019-contrato-do-renderizador-final.md):

```text
y = floor((54 * r + 183 * g + 19 * b + 128) / 256)
rgb = (y, y, y)
```

O Processador aplica essa operação em RGB não associado e preserva o alfa. O
Canvas WebGL usa um filtro da Foto, com conversão entre alfa premultiplicado e
não associado e o mesmo arredondamento inteiro. O filtro é liberado com seu nó;
a textura compartilhada permanece disponível para as outras colocações.
O SVG usa os mesmos pesos com `color-interpolation-filters="sRGB"`, preserva o
alfa e mantém IDs distintos entre instâncias. As prévias por paleta também
refletem o efeito quando a representação da imagem está indisponível.

Esta entrega mantém o amostrador e o protocolo de Exportação do fluxo atual;
não antecipa a substituição pelo compositor completo Q32.32 do Programa 04.
A versão da IPC do Processador passa de 19 para 20 porque `ComposedPhoto` ganha
um campo obrigatório. Um processador anterior não pode aceitar o comando e
ignorar silenciosamente o efeito.

## Arquivo v6

O objeto `photo.transform` da versão 6 acrescenta o booleano obrigatório
`blackAndWhite` aos campos da versão 5. A cadeia tipada
`v1 → v2 → v3 → v4 → v5 → v6` acrescenta `false` na passagem de v5 para v6,
preservando todos os ajustes existentes.

Abrir arquivos anteriores migra somente em memória. A revisão salva, o Histórico
e os bytes do arquivo permanecem intactos. O Salvamento explícito grava v6.
Cada versão mantém seu DTO fechado: v5 rejeita `blackAndWhite`; v6 rejeita campo
ausente, tipo diferente de booleano ou campo desconhecido. A reidentificação de
uma cópia mantém o esquema de origem até o Salvamento explícito.

## Verificação

As fronteiras públicas do Core cobrem ocorrência independente, seleção mista,
validação atômica, Histórico, cópia, troca, substituição, migração, Salvamento,
reabertura e snapshot de Exportação. O Processador real exporta cores conhecidas
e compara os tons de cinza esperados, a outra ocorrência colorida da mesma mídia,
a Borda verde e os bytes preservados do Original.

O corpus visual é produzido por comandos públicos do Core. A aceitação cobre
Foto individual, seleção mista, ação coletiva, restauração das cores, teclado,
Histórico, seção recolhida, prévias SVG, paleta e escalas de 100%, 125% e 150%.
Os contratos externos consultados foram os [filtros do PixiJS 8](https://pixijs.com/8.x/guides/components/filters)
(versão instalada 8.19.0) e a [matriz de cor SVG](https://developer.mozilla.org/en-US/docs/Web/SVG/Reference/Element/feColorMatrix),
incluindo o espaço de cor explícito das operações de filtro.
