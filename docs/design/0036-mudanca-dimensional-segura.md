---
status: accepted
document: design
date: 2026-09-16
ticket: 5
implementation-readiness: ready-for-agent
---

# Mudança dimensional segura

Este contrato conclui a decisão [#5](https://github.com/W4liss0n/my-Albuns/issues/5)
e orienta a implementação [#31](https://github.com/W4liss0n/my-Albuns/issues/31).
O [ADR 0011](../adr/0011-limitar-mudanca-de-proporcao-a-dez-por-cento.md) possui
o limite aprovado; a [SPEC](../specs/programa-de-diagramacao-de-albuns.md) possui
o comportamento observável. O código atual ainda bloqueia mudanças de tamanho
em Álbuns com Frames e proporções diferentes em Álbuns vazios. Este documento
não afirma que a implementação já foi entregue.

## Admissão do formato

Sejam `W0, H0` as dimensões atuais da Lâmina aberta e `W1, H1` as desejadas,
todas em micrômetros positivos. A largura permanece par, conforme o contrato
existente de divisão da Página. Validar primeiro medidas, DPI e margens.

```text
A = W1 × H0
B = W0 × H1
permitido = 10 × max(A, B) ≤ 11 × min(A, B)
diferença = max(A, B) / min(A, B) − 1
```

A comparação inteira é autoritativa; não usa a porcentagem arredondada da
interface. Produtos usam aritmética verificada com amplitude suficiente para
o intervalo canônico. A regra é simétrica e inclui exatamente 10%. Não muda
com a Unidade de apresentação ou o DPI. Não troca eixos nem gira conteúdo.
Formatos próximos de quadrado podem mudar o rótulo de orientação e continuar
válidos, como `23 × 24 → 24 × 23 cm`, com diferença de aproximadamente `8,88%`.

A base é a revisão atual, não um formato original oculto. Cada nova mudança
compara o estado confirmado com o destino solicitado. Mudanças sucessivas
podem chegar a outro formato por decisões explícitas; o limite não é uma
garantia semântica sobre rostos ou objetos dentro das Fotos.

## Geometria e medidas

Definir `sx = W1 / W0` e `sy = H1 / H0`. Cada Frame conserva seu ID, Foto ou
placeholder, Pilha visual e vínculo de estilo. Posições horizontais e bordas
verticais usam `sx`; posições verticais e bordas horizontais usam `sy`.
Uma Página única usa coordenadas locais à superfície ativa, com metade da
largura da Lâmina e a mesma altura. Não acrescentar deslocamento para o lado
inativo. Travessias centrais continuam pertencendo à superfície dupla.

Transformar as bordas, não somar larguras já arredondadas:

```text
Q(v, origem, destino) = floor((v × destino + floor(origem / 2)) / origem)
x1     = Q(x0, W0, W1)
right1 = Q(x0 + width0, W0, W1)
y1     = Q(y0, H0, H1)
bottom1= Q(y0 + height0, H0, H1)
width1 = right1 − x1
height1= bottom1 − y1
```

Para Página única, usar as larguras da superfície ativa em `Q`. Bordas que
coincidem usam o mesmo cálculo e continuam coincidindo. Validar todos os
retângulos finais: eixos positivos, contenção e preservação do lado ou da
Travessia central. Se a precisão inteira fizer um Frame degenerar ou eliminar
uma travessia existente, recusar o conjunto; não remover nem reorganizar Frames
para fazer o resultado caber. Não impor como nova restrição de persistência
o piso de tamanho usado apenas pelas alças de edição.

| Propriedade | Transformação |
|---|---|
| Frames e distâncias já presentes na composição | Escala por eixo, calculada pelas bordas |
| Borda padrão e espessuras próprias | Escala uniforme `min(sx, sy)`, arredondada ao micrômetro mais próximo |
| Margem, intervalo e menor lado configurados para Layouts | Mesma escala uniforme; validar o domínio de cada parâmetro |
| Sangria e Área de segurança | Conservam seus valores físicos, salvo edição explícita no mesmo Aplicar |
| Cor, Opacidade e efeitos | Conservados |
| Background e Overlay | Conservam vínculo, herança e escopo; a área de desenho é derivada da nova superfície |

Usar o menor fator nas medidas escalares evita engrossar a Borda em relação
ao eixo que menos cresceu. Os intervalos reais entre Frames podem ser distintos
nos dois eixos; o parâmetro único do Gerador orienta organizações futuras e
não força uma reorganização nesta operação. Zero continua zero. Resultado
inválido de qualquer parâmetro recusa toda a mudança.

Backgrounds e Overlays continuam seguindo sua regra vigente de preencher a
região decorativa. Eles não recebem o algoritmo de enquadramento de Fotos.
Sangria e segurança permanecem internas, conforme o [ADR 0004](../adr/0004-manter-margens-dentro-da-dimensao-exportada.md).

## Fotos e ponto focal

As Fotos nunca recebem `sx` e `sy` como escalas independentes. Giro, Ângulo
fino, Espelhamento, preto e branco e Zoom do usuário permanecem. O
`CompositionCore` recalcula o Zoom de preenchimento para o Frame transformado.
Uma mudança de proporção pode alterar o recorte, mesmo aceita pelo limite.

O ponto focal usado por esta operação é o ponto da Foto que estava no centro
do Frame antes da mudança, não uma face detectada. Ele é derivado da composição
atual, sem campo persistente novo ou ferramenta adicional. Conservar esse ponto
no centro do novo Frame sempre que for compatível com seu Preenchimento.

Na mesma proporção, conservar o Pan normalizado e o Zoom do usuário; a escala
uniforme do Frame é absorvida pelo Zoom de preenchimento. Não exigir Originais
nem metadados adicionais apenas para esse caso. A quantização micrométrica das
bordas continua sujeita às validações geométricas.

Para proporção diferente, calcular o Pan do destino com as mesmas dimensões
orientadas da Foto e os mesmos eixos de Pan usados no plano de composição:

```text
b0, b1 = Zoom de preenchimento antes e depois
u      = Zoom do usuário, conservado
L0, L1 = folga total disponível no eixo de Pan, antes e depois
p0     = Pan normalizado anterior nesse eixo
alvo   = p0 × L0 / 2 × (b1 / b0)
p1     = clamp(2 × alvo / L1, −1, 1), se L1 > 0
p1     = 0, se L1 = 0
```

Repetir nos dois eixos orientados. `L` é a dimensão da Foto desenhada com
`b × u`, menos a dimensão necessária para cobrir o Frame naquele eixo, nunca
negativa. Giro, Ângulo e Espelhamento participam desse cálculo pelo caminho
canônico existente; não duplicar suas fórmulas na interface. Se não houver
folga ou o alvo ultrapassar seu limite, usar a posição válida mais próxima.
Não aumentar silenciosamente o Zoom do usuário para forçar o ponto focal.
Quantizar o Pan na precisão persistente existente e manter seus limites.

Quando uma Foto colocada não tiver dimensões orientadas conhecidas, uma mudança
de proporção fica indisponível até obter essa observação. Metadados conservados
de uma mídia ausente são suficientes; não é necessário ter seu Original
disponível. Um placeholder não exige metadados. Não usar dimensões genéricas
do placeholder de Arquivo ausente para calcular um enquadramento definitivo.
O proprietário operacional pode recuperar a observação pelo fluxo existente;
o cálculo puro recebe os fatos imutáveis e não faz I/O.

## Layouts e catálogos

A transformação global não consulta o Gerador nem troca a organização.
Frames permanecem na mesma ordem e a permissão de Layout por Página ou por
Lâmina não muda. O estado travado é conservado. Esta é uma exceção explícita
à proibição de editar diretamente a geometria de uma Lâmina travada: somente
o resultado completo, validado pelo proprietário da mudança dimensional,
pode alterar suas medidas. O guard de travamento não deve aceitar qualquer
edição apenas porque as dimensões do documento também mudaram.

O Último Layout aplicado conserva sua origem e sequência de posições. Sua
definição é adaptada à nova proporção pela mesma transformação relativa,
preservando o escopo. Usar a superfície de referência da própria definição
como origem e a superfície nova correspondente como destino; não substituir
suas posições pelas edições manuais atuais dos Frames. Para uma definição
histórica incompatível com a superfície anterior, conservar a cópia como
incompatível, sem atribuir-lhe uma correspondência inventada. A preview de
uma Lâmina travada continua refletindo os Frames efetivos, conforme o design 0028.

Favoritos do Projeto e Layouts personalizados globais conservam suas definições
independentes. O mecanismo atual continua filtrando e mapeando candidatos
compatíveis; esta operação não amplia a regra geral de compatibilidade dos
catálogos. Uma nova proporção pode tornar um Favorito incompatível e deixar
o Último Layout transformado sem a mesma identidade geométrica do catálogo.
Isso não apaga o Favorito nem modifica outros Projetos.

## Unidade, DPI e aplicação

Trocar somente a Unidade converte a apresentação imediatamente e preserva
todos os valores físicos. Trocar somente DPI conserva composição, Pan e Zoom;
altera a resolução derivada da Exportação, sem exigir regenerar o Cache de
mídias cuja representação não depende desse DPI.

O fluxo reutiliza Informações do Álbum e seu único `Aplicar`:

1. O rascunho usa os valores canônicos; pré-validação consulta o Core e retorna
   os impedimentos, sem modificar Projeto ou Histórico.
2. A revisão calcula o candidato completo a partir do estado autoritativo e
   apresenta as dimensões físicas e a resolução final. Se a proporção mudar,
   informa concisamente que o recorte das Fotos poderá ser ajustado.
3. Cancelar descarta a aplicação. Confirmar entra na fila existente de mutações.
4. Na execução, materializar o delta sobre a revisão vigente. Se dimensões,
   composição ou observações de mídia usadas pelo cálculo mudaram, recalcular
   e apresentar novamente o impacto materialmente diferente, sem confirmar
   silenciosamente outro resultado.
5. Validar todo o candidato e publicar uma única revisão/ação de Histórico.
   Undo restaura os valores anteriores exatos; Redo reaplica o candidato já
   confirmado, sem depender de novo acesso aos Originais.

Se o mesmo Aplicar também converter uma extremidade, primeiro transformar as
superfícies existentes no candidato e depois executar a conversão pelo dono
atual desse comando. A confirmação inclui as perdas previstas pela conversão.
A conversão conserva sua regra própria de destravar/reorganizar a extremidade;
isso não destrava as demais Lâminas. Nenhum estágio intermediário é publicado.

Não criar outra janela, faixa temporária ou aviso fixo no painel. Bloqueios
permanecem nos tooltips dos campos e a confirmação usa o diálogo vigente.
Somente Salvar publica o arquivo. Cancelamento, validação inválida, falha de
preparação ou confirmação vencida preservam a composição e o Histórico.

O ProjectCore possui admissão, quantização e preparação da transformação;
CompositionCore fornece o mesmo enquadramento ao Canvas e à Exportação. A
interface não serializa Frames transformados nem replica o limite em uma
segunda regra autoritativa. Campos persistentes e seus significados permanecem;
o ponto focal é derivado, portanto esta decisão não exige evolução do schema.

## Casos de referência

As medidas da tabela são da Lâmina fechada, em centímetros. Todos os pares são
testados também no sentido inverso. O percentual é apenas apresentação.

| Atual | Destino | Diferença | Resultado |
|---|---|---:|---|
| 20 × 20 | 30 × 30 | 0% | Aceito |
| 20 × 30 | 30 × 45 | 0% | Aceito |
| 20 × 30 | 25 × 35 | 7,142857…% | Aceito |
| 25 × 35 | 30 × 40 | 5% | Aceito |
| 25 × 30 | 30 × 35 | 2,857142…% | Aceito |
| 30 × 40 | 30 × 44 | 10% | Aceito, limite inclusivo |
| 30 × 40 | 30 × 45 | 12,5% | Recusado |
| 20 × 30 | 30 × 20 | 125% | Recusado |
| 30 × 30 | 30 × 33 | 10% | Aceito, limite inclusivo |
| 30 × 30 | 30 × 33,0001 | 10,000333…% | Recusado, mesmo exibindo 10,00% |
| 23 × 24 | 24 × 23 | 8,884688…% | Aceito, sem girar conteúdo |

Referência de Pan sem Giro/Ângulo: Foto de `1200 × 800 px`, Frame de
`120 × 80 mm`, Zoom do usuário `2` e Pan `(0,5; −0,25)`. Ao mudar o Frame
para `126 × 80 mm`, `b0 = 0,1 mm/px`, `b1 = 0,105 mm/px`; o novo Pan é
`(0,5; −0,238636…)`, quantizado na precisão vigente. O mesmo ponto da Foto
permanece no centro. Com Foto `1200 × 800 px`, Frame `100 × 100 mm`, Zoom `1`
e Pan horizontal `1`, mudar o Frame para `110 × 100 mm` exigiria Pan `1,25`;
o resultado é `1`, preservando o Preenchimento e ajustando o recorte.

A implementação deve ainda comprovar, pela fronteira pública do Core:

- composição inteira, Página única dos dois lados, travessias, placeholders,
  alinhamentos compartilhados e quantização que recusa degeneração;
- Borda herdada/própria, parâmetros de Layout, margens de acabamento invariantes,
  Background e Overlay por lado e em Ambos os lados;
- Foto com Giro, Ângulo e Espelhamento, Pan no centro e nos limites, folga zero,
  Foto ausente com metadados e ausência real de metadados;
- Layout travado preservado, Último Layout adaptado, Favoritos e catálogo global
  intactos, e conversão de extremidade no mesmo Aplicar;
- nenhuma alteração parcial em falha, comandos adjacentes pendentes, uma ação
  de Undo/Redo e resultado obsoleto sem confirmação silenciosa;
- Salvar/reabrir conservando o resultado e equivalência com a Exportação;
  Unidade e DPI isolados não alterando composição nem consumindo a regra de 10%.
