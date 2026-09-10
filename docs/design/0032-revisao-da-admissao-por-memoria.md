---
status: accepted
document: design
date: 2026-09-10
platform: windows
implementation-readiness: ready-for-agent
---

# Revisão da admissão por memória

## Escopo

`ImagingProcessor` usa a memória disponível para ajustar o paralelismo e
permite processamento individual quando a próxima imagem couber com uma
margem de segurança. Este contrato substitui a espera indefinida sob pressão
descrita anteriormente em
[Importação com decode único e lotes](0020-importacao-com-decode-unico-e-lotes.md).
Importação, Cache e inspeções compartilham o mesmo proprietário da admissão.

## Problema da política anterior

A admissão acumula uma estimativa conservadora por Original, uma margem de
até 2 GiB e a utilização de somente metade da RAM física e do commit restantes.
Isso pode impedir qualquer trabalho mesmo quando uma execução individual
seria viável. Exibir apenas “Aguardando memória disponível” explicaria a
espera, mas não resolveria essa restrição.

Na tentativa observada em 10/09/2026, as 44 imagens tinham estimativas entre
224 e 236 MiB por Original. Uma medição encontrou 1,66 GiB de RAM disponível,
mas a regra exigia aproximadamente 2,46 GiB para admitir o maior Original.
Esse último valor é um limiar da política, não um consumo medido da imagem.
Depois de liberar memória, o usuário confirmou que a importação prosseguiu.
As falhas posteriores de miniaturas vieram de identificadores de componentes
JPEG com declaração Adobe válida; o contrato corrigido está em
[Contrato JPEG do primeiro fluxo](0014-contrato-jpeg-do-primeiro-fluxo.md).

O teste existente de espera, recuperação de recursos e cancelamento confirma
que o código cumpre a regra; ele não valida a adequação dos limites escolhidos.

## Política de admissão

O teto agregado continua sendo o menor valor entre um quarto da RAM física
total e 4 GiB. A estimativa por Original continua incluindo 16 bytes por
pixel, duas vezes o tamanho comprimido e 64 MiB para buffers e processo.
Um lote mantém um Original por vez e reserva o máximo de suas estimativas.

Para iniciar trabalhos simultâneos, preserva-se a margem anterior de um
oitavo da RAM total, limitada entre 512 MiB e 2 GiB, usando somente metade
da RAM e do commit restantes. As reservas ativas são descontadas desse
orçamento: um trabalho admitido pode ainda não ter alocado seus buffers.

Quando essa folga não permite paralelismo e não existe reserva ativa, pode
ser admitido um único trabalho se sua estimativa couber na RAM física e
no commit disponíveis depois de preservar 512 MiB, sem a divisão por dois.
O piso de 512 MiB já fazia parte da política; a mudança retira as margens
adicionais exigidas para concorrência quando há somente um trabalho.
O teto agregado e os limites do decoder continuam valendo.

Enquanto houver trabalho ativo, a fila aguarda de forma cancelável e reavalia
os recursos a cada 100 ms. A conclusão libera a reserva e permite continuar
serialmente ou voltar ao paralelismo se houver folga. Não se interrompe um
decoder iniciado apenas porque a pressão externa mudou.

Se nenhum trabalho estiver ativo e a estimativa não couber nem individualmente,
a admissão retorna `MemoryPressure` imediatamente. Essa falha não suspende nem
coloca o Processador em quarentena. Uma imagem acima do teto continua recebendo
`MemoryLimit`. Ausência de telemetria permite apenas uma reserva até 1 GiB.

## Resultado e nova tentativa

A operação termina o progresso e apresenta o motivo na Tela de Problemas.
Imagens já validadas são preservadas pelo resultado parcial normal, em uma
única ação de Histórico. Uma imagem que não pôde ser validada por falta de
recursos não recebe um vínculo fictício nem é marcada como corrompida.

Depois de liberar memória, a pessoa pode executar novamente `Importar` por
arquivos, pasta ou arraste. Essa é uma nova tentativa: as imagens já vinculadas
seguem a regra existente de reimportação, sem duplicação. Para uma imagem
vinculada cuja miniatura esteja indisponível, o menu mantém `Tentar novamente`.
Não há retomada automática de uma ação já encerrada nem retenção oculta da
seleção de arquivos depois do término.

## Verificação e observabilidade

- Admitir uma imagem estimada em 236 MiB com 1.699 MiB físicos e 3.393 MiB de
  commit disponíveis; manter o próximo trabalhador em espera até a liberação.
- Concluir a importação real por arquivos e por pasta com um trabalhador sob
  a mesma pressão simulada, preservando todas as prévias e o progresso.
- Recusar prontamente a falta de RAM ou commit até para um trabalho, permitir
  nova tentativa após a recuperação e preservar resultados parciais sem duplicar.
- Manter o teto agregado, os limites do decoder, o cancelamento, a quarentena
  e a confirmação de encerramento antes de devolver recursos.
- Registrar `processor_memory_wait_started`, `processor_memory_admitted` e
  `processor_memory_unavailable`, com estimativas e recursos pertinentes.

As margens são limites operacionais conservadores, não uma garantia contra
alocações de outros programas depois da leitura. Medições do corpus aceito
devem acompanhar a verificação; mudanças futuras da estimativa exigem nova
evidência, sem aprendizado automático pela velocidade de uma importação.

### Medição dos JPEGs afetados

Em 10/09/2026, o Processador debug corrigido preparou individualmente duas
imagens reais de 3.603 × 2.776 pixels. Os picos do processo filho, consultados
por `GetProcessMemoryInfo` a cada 10 ms, foram:

| Original do corpus | Estimativa de admissão | Pico de working set observado | Pico de commit observado |
| --- | ---: | ---: | ---: |
| `001.jpg` | 227,1 MiB | 49,2 MiB | 40,1 MiB |
| `029.jpg` (maior arquivo comprimido) | 235,6 MiB | 53,1 MiB | 43,9 MiB |

Ambas produziram prévias de 1.600 × 1.233 pixels, com SHA-256 dos Originais
inalterado antes e depois. Essa amostra confirma a folga da estimativa para os
JPEGs observados; não caracteriza todos os codecs, o consumo do Host ou de
outros programas. Os contadores são picos observados durante amostragem e podem
omitir atividade posterior à última consulta, antes do encerramento do filho.

O teste pelo fluxo nativo completo importou as 44 imagens em 7,10 segundos,
com 44 prévias publicadas e reutilizadas, nenhuma rejeição, nenhum decode
alternativo no Host e SHA-256 dos 44 Originais preservado. Essa execução usou
um orçamento fixo com folga para separar o sucesso funcional da pressão de
outros programas. Outro teste executou os processos reais por arquivos e por
pasta com 1.699 MiB físicos e 3.393 MiB de commit simulados, confirmando pico
de um trabalhador e conclusão de todas as prévias.

Na confirmação pelo aplicativo Windows, a pasta foi selecionada pelo diálogo
nativo. A telemetria real registrou admissões seriais com RAM disponível entre
1.289 e 1.865 MiB. A operação importou 44 imagens, sem rejeições ou falhas do
Processador; a janela exibiu avanços intermediários e as 44 miniaturas foram
carregadas, incluindo após rolar o Painel. Capturas e registros locais dessa
verificação estão em `.scratch/memory-adobe-native-proof/`.

## Contratos externos consultados

O contrato usado pelo projeto é Win32, por meio de `windows-sys` 0.61.2.
A documentação de
[PERFORMANCE_INFORMATION](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-performance_information)
distingue RAM física imediatamente reutilizável de capacidade de commit.
[CreateMemoryResourceNotification](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-creatememoryresourcenotification)
documenta sinais de pressão que permitem ajustar o consumo, inclusive uma
faixa intermediária em que nenhum dos sinais está ativo. Esta implementação
mantém a consulta a `GetPerformanceInfo`; não acrescenta outra fonte de
admissão. Nenhuma dessas leituras garante o sucesso de uma alocação futura.

`find-docs` foi acionado, mas o Context7 estava sem cota. A consulta foi feita
diretamente nas páginas oficiais da Microsoft em 10/09/2026.
