---
status: accepted
document: design
date: 2026-09-17
platform: windows
implementation-readiness: ready-for-agent
---

# Fatos do Core e reutilização dos controles do editor

A revisão da fronteira UI/Rust identificou quatro oportunidades, aprovadas em
conjunto pelo autor. Esta decisão complementa os designs 0037 e 0038 e preserva
o ADR 0005: a UI possui interação e apresentação; o Core possui regras documentais.

## Estrutura das Lâminas

Cada Lâmina projetada contém a disponibilidade de inserção, duplicação, exclusão
e conversão e um intervalo de posições válidas para reordenação. `ProjectDocument`
produz esses fatos com as regras que os comandos usam. Não há nova chamada IPC
por movimento do ponteiro. A UI consulta a projeção vigente, mantém ghost,
placeholder, seleção e modo de edição e rematerializa o destino por vizinhos
quando um comando anterior altera a sequência. A elegibilidade é conferida
novamente com a projeção autoritativa ao executar a fila.

Esses fatos não são configuração persistente nem estado visual: são resultados
derivados do documento. O arquivo do Projeto e sua versão não mudam.

## Perdas da conversão de extremidade

A operação que retém os lados ativos informa as aplicações que removeu. A
projeção da Lâmina e a validação de Informações do Álbum obtêm os fatos de perda
por essa mesma transformação, sem alterar o documento. Herdados, Ambos os lados,
o lado preservado e Overlay explicitamente ausente não geram perda efetiva.

A UI recebe Identidade, número, lado e conteúdo removido. Textos, decisão do
usuário e comparação do conteúdo apresentado continuam na UI. Os consumidores
mantêm a revalidação e o ciclo de decisão do design 0038; fatos novos depois de
Undo ou outra alteração exigem nova confirmação. A consulta não cria Histórico.

## Controles existentes

O Zoom individual passa a compor `PhotoZoomControl`, o mesmo controle numérico
usado na seleção múltipla. Oferece campo percentual inteiro, slider, validação,
Escape e restauração ao mínimo por dois cliques no slider. Mantém o estilo,
a tipografia, o foco neutro, a escala e os tokens dos controles de propriedades;
não há nova paleta, superfície, hierarquia ou sistema de espaçamento.

A apresentação compartilhada preserva dois comandos: individual envia
`transformPhoto`, com delta de quatro casas e sem alterar Pan; seleção múltipla
envia `setPhotoZoom` absoluto. A prévia individual continua acompanhando o Canvas.
Enquanto seu delta aguarda a fila, o controle fica desabilitado. Troca de seleção,
cancelamento e conclusão tardia não recuperam o rascunho antigo.

`NumericPropertyControl` e `ColorPropertyControl` passam a compor `TextInput` nos
campos textuais internos. Seus drafts, refs, validação, foco e handlers permanecem
com os controles existentes. A política comum de autocomplete não admite exceção
nesses campos. Inputs de cor e range permanecem nativos.

## Verificação

- Projeção estrutural versus comandos reais: duas, três e cinco Lâminas; quatro
  combinações de extremidades; cada origem e posição, incluindo fora do intervalo;
  rejeição atômica e restauração por Undo.
- Fatos de perda versus conversão direta e Informações do Álbum: ambas as
  extremidades, Background, Overlay, ausência explícita, Ambos os lados, herança,
  lado preservado e expansão; consulta sem mutação e reversibilidade.
- UI: rematerialização da fila, reconfirmação, sucesso/falha pendentes, mudança de
  seleção/Projeto, entrada numérica, inválido, Escape, restauração e gesto único.
- Capturas dos cenários afetados no manifesto vigente, sem substituir a referência.

## Contrato externo consultado

React instalado: 19.2.8. Context7 estava indisponível por cota. Foram consultados
os contratos oficiais de [input controlado](https://react.dev/reference/react-dom/components/input)
e [limpeza de Effects](https://react.dev/reference/react/useEffect). Mantém-se a
atualização síncrona dos drafts e a limpeza de listeners/temporizadores do controle
existente; não se introduz API nova nem dependência.
