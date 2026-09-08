---
status: accepted
document: design
date: 2026-09-08
ticket: 22
---

# Orientação de Fotos e Arquivo de Projeto v4

Este recorte da issue #22 implementa o Giro em passos de 90° anti-horários e o
Espelhamento horizontal definidos na especificação. O Painel contextual oferece
as duas ações para uma Foto e para as Fotos de uma seleção múltipla. Frames sem
Foto não recebem transformações e, quando nenhum selecionado contém Foto, esses
controles ficam ocultos.

## Comandos e apresentação

`Girar 90° à esquerda` avança o Giro comum em um quarto de volta. Quatro ativações
retornam à orientação inicial. O valor exibido usa o sentido anti-horário:
`0°`, `90°`, `180°`, `270°`. `Restaurar giro` retorna somente esse ajuste a `0°`
e fica indisponível quando todas as Fotos selecionadas já estão nessa posição.
O Espelhamento é um controle binário independente do Giro.

Na seleção múltipla, o Painel mostra o alcance, por exemplo `Aplicado a 2 Fotos
de 3 Frames`, sem eleger uma Foto como preview. Giros diferentes mostram `—`;
Espelhamentos diferentes mostram um estado neutro. Como exige o design 0001,
o primeiro ajuste explícito aplica um valor absoluto comum: Girar a partir de
valores divergentes escolhe `90°` anti-horários; Espelhar escolhe ligado. A
restauração escolhe `0°` para todas. Essa decisão é calculada sobre o estado
autoritativo no Core, inclusive em ativações adjacentes antes da primeira resposta.

Cada comando é atômico, preserva seleção, navegação, identidade e ordem dos
Frames, geometria, vínculos, Pan e Zoom do usuário. Placeholders permanecem iguais.
Uma seleção inválida é rejeitada inteira. Restaurar um valor já neutro ou aplicar
uma ação somente a placeholders não cria Revisão, alteração ou Histórico e não
descarta Refazer. A interface oculta ou desabilita esses casos conforme descrito.

Os comandos compartilham a fila da Janela com Salvar e Histórico. Os IDs capturados
não mudam quando o usuário troca a seleção durante a espera. Falhas cancelam os
comandos adjacentes dependentes. Substituir uma Foto continua reiniciando todos os
ajustes da nova ocorrência; Copiar/Colar e Trocar conteúdo levam a orientação junto
da ocorrência existente.

## Composição

O Core continua proprietário do Preenchimento do Frame e dos limites de Pan.
A ordem é Giro, Espelhamento horizontal, Zoom de preenchimento, Zoom do usuário
e Pan, preservando o espaço reservado ao Ângulo fino entre Giro e Espelhamento.
Esse Ângulo continua pertencendo a um recorte posterior da issue #22.

O Espelhamento inverte a horizontal da Foto já girada. Canvas, miniaturas e
Exportação obedecem à mesma ordem. No Pixi, cujo `scale` atua nos eixos locais,
isso equivale a inverter o sinal do ângulo quando a escala horizontal é negativa.
O renderizador final inverte primeiro o Espelhamento e depois o Giro ao mapear
um pixel de saída para o Original. Pan permanece aplicado depois dessas operações.

## Formato público v4

O ADR 0009 exige nova versão para novos campos persistentes. `schemaVersion: 4`
mantém envelope, Identidade, Revisão, Lâminas, Frames e mídias da v3, acrescentando
dois campos obrigatórios ao transform de cada Foto:

```text
transform: { panX, panY, userZoom, quarterTurns, mirrorX }
```

`quarterTurns` é inteiro entre `0` e `3`, no sentido positivo horário usado pela
composição; `3` corresponde a `90°` anti-horários na interface. `mirrorX` é
booleano. A v4 não persiste Ângulo fino nem propriedades ainda não implementadas.
Pan e Zoom conservam seus contratos anteriores. Observações de mídia, Originais,
Cache, seleção e estado do Canvas continuam fora do Arquivo de Projeto.

A cadeia tipada `v1 → v2 → v3 → v4` ocorre somente em memória. A última etapa
acrescenta `quarterTurns: 0` e `mirrorX: false`, preservando todos os valores v3.
Abrir não regrava nem cria Histórico ou alteração criativa. Salvar explicitamente
promove o arquivo para v4 pela publicação atômica existente, sem nova Revisão
criativa. `Salvar como`, Cópia externa e Recuperação conservam os mesmos contratos
de Identidade e autorização de escrita.

Os DTOs continuam fechados e separados do domínio e do IPC. As formas estáveis
do envelope com Frames são compartilhadas por tipos genéricos, mas os transforms
v3 e v4 são distintos: o leitor v3 continua rejeitando campos v4. Campos
desconhecidos, obrigatórios ausentes, Giros fora do intervalo, tipos inválidos e
versões futuras são recusados sem alterar a origem.

## Exemplos e verificação

- [Entrada v3 com Foto e placeholder](../../crates/myalbuns-core/tests/fixtures/project_document_v3_photo_migration_input.myalbuns).
- [Resultado esperado da migração v3 → v4](../../crates/myalbuns-core/tests/fixtures/project_document_v4_photo_migration_expected.myalbuns).
- [Resultado atual da cadeia iniciada em v1](../../crates/myalbuns-core/tests/fixtures/project_document_v4_migration_expected.myalbuns).
- `photo_orientation.rs` verifica comandos públicos, seleção mista, casos inválidos,
  Histórico, preservação do Original, Salvamento, reabertura, snapshot de Exportação,
  migração e rejeição de documentos inválidos. Também produz e verifica o corpus
  utilizado pelas capturas da interface.
- A fronteira pública do Processador verifica a permutação esperada de quatro
  quadrantes coloridos em JPEGs exportados, incluindo Giro seguido de Espelhamento.
- A integração da Janela verifica comandos adjacentes, sucesso/falha, seleção
  alterada durante a espera, Modo normal e bloqueios de interação.
