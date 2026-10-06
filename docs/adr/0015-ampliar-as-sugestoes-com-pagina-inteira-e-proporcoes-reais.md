---
status: accepted
date: 2026-10-01
---

# Ampliar as sugestões com Página inteira e proporções reais

A versão 2 do Gerador, aceita no [ADR 0013](0013-diversificar-e-harmonizar-as-sugestoes-do-gerador.md),
deixou em aberto cinco mudanças que criam composições novas ou mudam o que o
Gerador recebe. O autor pediu em 01/10/2026 que todas fossem feitas. A versão 3
mantém as famílias, a busca determinística e a seleção da versão 2, e acrescenta:

- **Proporção real da Foto.** Um Frame com Foto mira a proporção da Foto como ela
  é mostrada: dimensões observadas pela sessão, já corrigidas pela orientação EXIF,
  trocadas por um Giro de um quarto de volta. Fotos de celular (3:4, 9:16) e de
  câmera (2:3, 4:5) deixam de ser tratadas todas como 2:3 ou 3:2. A proporção só
  vale quando tem a orientação do Frame e fica um pouco dentro das proporções que
  essa orientação admite; sem dimensões observadas, continua a referência 2:3,
  3:2 ou 1:1. A orientação de um Frame existente continua sendo a da sua
  geometria, como no [ADR 0014](0014-orientar-novos-frames-pela-foto-inserida.md).
- **Página inteira.** Uma sugestão pode levar a Foto de um Frame até as bordas de
  uma Página inteira, sem Margem, com uma composição normal dos demais Frames na
  outra Página. Numa Página única, um Frame sozinho pode ocupar toda a área ativa.
  Era o padrão de 20 dos 43 modelos "Limpo" do myAlbuns antigo. Em cada orientação,
  o Frame cuja Foto a Página corta menos é o candidato. O Frame toma a forma da
  Página, qualquer que seja sua orientação. Como a Sangria é interna à Lâmina
  ([ADR 0004](0004-manter-margens-dentro-da-dimensao-exportada.md)), cobrir a
  Página até a borda externa já inclui a Sangria. Essas sugestões têm sua própria
  janela de nota e não alinham horizonte com a outra Página. A escolha automática
  nunca aplica uma Página inteira; ela continua disponível no Painel.
  Atualização de 06/10/2026: um Layout personalizado salvo pela pessoa com uma
  Página inteira aplica-se automaticamente como foi salvo; a restrição vale
  para as sugestões do Gerador e para Favoritos automáticos (design 0026).
- **Frames vazios livres no Painel.** Nas consultas do Painel, um Frame vazio não
  tem orientação fixa: em cada sugestão, o Gerador o faz vertical ou horizontal.
  Isso vale para os placeholders existentes e para os criados pelo seletor de
  quantidade, que deixa de pedir sempre Frames horizontais. O Gerador busca até
  sete divisões, de nenhum a todos os Frames livres na vertical, em passos iguais,
  e todas competem numa única classificação. Frames com Foto conservam a
  orientação, e a escolha automática continua consultando a orientação atual de
  todos os Frames. Substitui a saída curta prevista para o seletor, que pediria a
  orientação predominante das Fotos do Projeto.
- **Blocos encaixados.** Numa região de três a seis Frames, o Gerador também corta
  o bloco em até quatro partes, cada parte na direção oposta, até três níveis.
  Com o intervalo fixo, a largura de cada parte é linear na sua altura, e as medidas
  saem exatas: cada Frame fica na proporção que mira, sem corte. É a generalização,
  para três níveis, do destaque resolvido junto com seu apoio na versão 2, e traz
  composições que as famílias fixas não fazem, como um destaque central entre duas
  pilhas. Até seis entram por região, pela nota do bloco.
- **Harmonia na nota.** O tamanho mínimo confortável passa a ser o maior entre duas
  vezes o menor lado pedido e 15% da altura da superfície. O limite de contraste,
  que valia para grupos de cinco ou mais Frames, passa a valer a partir de três.
  Grupos de até seis Frames perdem quatro pontos por tamanho além de três. Em
  grupos maiores, tamanhos graduados são intencionais. A versão 2 adiou essas
  penalidades porque, sozinhas, aumentavam o corte e reduziam a variedade. Com os
  blocos encaixados, que dão alternativas sem corte, o resultado melhora nas duas
  medidas. Contar os tamanhos também em grupos grandes empurrava para o fim as
  composições que atravessam o centro, sem que isso tivesse sido pedido; ficou de fora.

Nas 1.150 consultas da análise de 30/09/2026 (Frames com orientação fixa e
proporções de referência), em relação à versão 2:

| Medida | Versão 2 | Versão 3 |
| --- | --- | --- |
| Consultas com menos de cinco sugestões | 164 | 129 |
| Estruturas distintas por lista | 13,9 | 14,8 |
| Primeira sugestão com corte acima de 20% | 11,2% | 10,6% |
| Primeira sugestão com contraste de tamanho acima de 6 | 42,0% | 40,4% |
| Primeira sugestão com lado curto abaixo de 15% da altura | 21,0% | 18,3% |
| Todas as sugestões com contraste acima de 6 | 59,0% | 54,1% |

A Página inteira aparece em 500 das 1.150 listas e é a primeira em 84, das quais 67
em Lâminas de 40 × 30 cm, cujas Páginas têm a proporção de uma Foto vertical. Nas
demais, aparece em torno da quarta posição. Os blocos encaixados respondem por 221
sugestões e são os primeiros em 44 listas, além de comporem Páginas. Com Fotos de
proporções diferentes, a mudança aparece mais: cinco Fotos verticais de 3:4, 2:3 e
9:16 numa Página de 30 × 30 cm tinham uma sugestão; agora têm quatro. Seis Frames
vazios numa Lâmina de 60 × 30 cm tinham sete sugestões, todas horizontais; agora
têm vinte, com 0 a 6 verticais.

Em contrapartida:

- a Página inteira quebra a regra de que toda sugestão respeita a Margem;
- a consulta do Gerador passa a incluir a proporção de cada Foto e Frames sem
  orientação;
- o pedido de quantidade do Painel deixa de carregar orientação;
- a busca fica mais demorada. Nas 1.150 consultas, com o núcleo otimizado do
  aplicativo de desenvolvimento, a média passa de 1,4 para 3,8 ms com 6 Frames,
  de 8,2 para 17,3 ms com 12 e de 17,2 para 22,2 ms com 20; a consulta mais lenta
  leva 62 ms (44 ms na versão 2). Cada divisão de Frames livres repete a busca:
  4 Frames vazios levam cerca de 5 ms, 6 entre 25 e 50 ms, 12 cerca de 0,1 s e
  30 cerca de 0,2 s. As soluções de cada região são reaproveitadas dentro da
  mesma busca, e o resultado não depende de qual delas foi calculada primeiro.
  Uma consulta repetida continua reaproveitada na sessão.

O autor aprovou a versão 3 em 01/10/2026, depois de comparar as sugestões das
duas versões lado a lado. Detalhes e parâmetros estão no
[design 0026](../design/0026-contrato-do-gerador-e-da-aplicacao-de-layouts.md).
