---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-30
updated: 2026-07-31
---

# Isolamento e recuperação de processos

## Pergunta

Este gate precisava comparar o domínio de falha das duas topologias sem
antecipar eleição, watchdog ou reinício automático:

- as Janelas do Projeto continuam operacionais quando o processo global cai;
- uma edição documental real pode ser salva e relida enquanto as operações
  globais permanecem indisponíveis;
- o reinício global é explícito, troca o PID e continua protegido por
  instância única;
- a queda de um host preserva somente as Janelas que pertencem a outro host;
- um host reiniciado reabre a última revisão explicitamente salva;
- a recuperação já demonstrada do Processador de Imagens pertence ao mesmo
  commit;
- a IPC e os logs oferecem correlação suficiente para explicar cada
  resultado, sem substituir relações observáveis por um escore inventado.

## Contrato exercitado

O executável medido possui um papel global exclusivo do spike. Quando
`MYALBUNS_PROCESS_ROLE=global`, ele desvia antes de construir Tauri,
`ProjectHost`, `ProjectSession`, WebView, Canvas ou Cache. Nesse papel, mantém
somente um servidor tipado em TCP loopback. O bind do endpoint funciona também
como proteção de instância única; uma segunda cópia termina com o código
estável `73` e não desloca a proprietária.

Esse transporte aceita apenas uma consulta de status em uma linha JSON,
correlacionada por execução, topologia e probe. Ele existe para tornar
disponibilidade e propriedade mensuráveis. Não é uma escolha de IPC para o
produto.

Cada Janela continua ligada à sua única `ProjectSession` mutável. O probe de
continuidade atravessa quatro comandos reais do frontend para o host:

1. `topology_fault_probe_config`;
2. `project_state`;
3. `apply_project_intent`, com uma intenção documental `TransformPhoto`;
4. `persist_topology_fault_probe`.

O host serializa a revisão pelo núcleo, grava um temporário irmão, executa
`flush` e `sync`, promove por rename, relê os bytes por
`ProjectCore::load_persisted_revision` e verifica identidade, revisão,
tamanho e SHA-256. Somente depois confirma a revisão da Sessão como salva e
`dirty: false`. A quinta interação correlacionada é a consulta do host ao
processo global.

No reinício do host, o runner fornece como fonte a última revisão publicada
nesse diretório descartável. A abertura passa pelo núcleo e precisa produzir
um evento estruturado com o mesmo Projeto e a mesma revisão. Essa costura é
instrumentação do spike; ela não implementa `ProjectStore`, autosave ou
`RecoveryStore`.

Antes de qualquer encerramento forçado, o runner resolve o PID e confirma que
seu executável é exatamente
`.scratch/topology-spike-target/release/myalbuns-desktop.exe`. Como um processo
encerrado não pode registrar a própria morte, horário, alvo, papel e observação
do término pertencem ao runner.

## Instrumento

A execução canônica usou perfil `release` no commit
`65919f632feb32a53a22b16db3cbc551d82653f6`. As 182 entradas da build estavam
limpas, tinham digest
`8b491979a3717ab92708fac35591a4e39cd4cb868787e555e890cb502cd1c30a`
e ainda correspondiam ao executável no fim da coleta. O checkout tinha
mudanças alheias às entradas medidas, registradas separadamente pelo
manifesto.

O host tinha SHA-256
`e28e86714a9c7e0331c3de236d1dfc743d96787f3e0e9426e80333888fde3802`;
o Processador de Imagens,
`5b90bfd5785c84e70224733cbc5b3cb85ee304f500e22afad33c56185f40222b`.

A máquina executava Windows 11 Pro `10.0.26200`, com Intel Core i5-13450HX,
24.260,7 MiB de memória física, NVIDIA GeForce RTX 3050 6GB Laptop GPU e
Intel UHD Graphics. O corpus continha dois Álbuns, 172 Fotos JPEG e um
Decorativo PNG, totalizando 173 mídias e 1.401,0 MiB. Seu SHA-256 permaneceu
`c160ef3e3a2a1c7401f10574e3383fddb830678eb3fbc984dbca28dc59ebd01f`
antes e depois das duas alternativas.

O mesmo cenário de 100 Lâminas por Projeto executou Cache, Canvas, Pan, Zoom,
navegação, perda e restauração WebGL2 e Exportação antes das falhas:

| Contexto da execução | A — hosts independentes | B — host multiwindow |
| --- | ---: | ---: |
| Hosts do Projeto | 2 | 1 |
| Janelas do Projeto | 2 | 2 |
| Processos nas árvores dos hosts | 14 | 8 |
| Working set agregado | 2.224,3 MiB | 1.950,3 MiB |
| Memória privada agregada | 1.670,7 MiB | 1.562,8 MiB |
| Janelas prontas | 4.730 ms | 2.457 ms |
| Cache frio pronto | 33.922 ms | 46.020 ms |

Em cada alternativa, a ordem posterior foi:

1. iniciar o processo global, consultar seu status e rejeitar uma duplicata;
2. encerrar o global, confirmar sua ausência e salvar os dois Projetos;
3. reiniciar explicitamente o global, rejeitar outra duplicata e salvar
   novamente os dois Projetos;
4. encerrar um host;
5. na topologia A, salvar mais uma vez pelo host sobrevivente;
6. reiniciar explicitamente o host encerrado, reabrir a última revisão salva e
   aguardar seu Cache antes da limpeza.

Os dados brutos e todas as métricas da execução estão no
[artefato JSON](artifacts/0007-process-failure-gate.json). O
[resumo gerado](artifacts/0007-process-failure-gate.md) mantém a comparação
geral do cenário.

## Resultados

### Processo global

O papel global real ocupou uma única árvore com um único processo, cinco
threads, 143 handles, nenhuma Janela e nenhum uso gráfico observado nas duas
alternativas.

| Evidência | A — hosts independentes | B — host multiwindow |
| --- | ---: | ---: |
| PID global inicial | `35512` | `34148` |
| Status inicial | 122,582 ms | 509,334 ms |
| Working set do global | 14,1 MiB | 14,1 MiB |
| Memória privada do global | 5,7 MiB | 5,7 MiB |
| Janelas próprias | 0 | 0 |
| PID da primeira duplicata rejeitada | `4220` | `8004` |
| Código da rejeição | `73` | `73` |
| Observação do término global | 8 ms | 4 ms |
| Confirmação de indisponibilidade | 532,057 ms | 531,763 ms |
| Janelas preservadas sem o global | 2 | 2 |
| PID global depois do reinício explícito | `5932` | `29912` |
| Status do global reiniciado | 510,997 ms | 514,589 ms |
| PID da segunda duplicata rejeitada | `14704` | `33368` |
| Código da segunda rejeição | `73` | `73` |

Depois de cada queda, o conjunto de processos do executável continha somente
os hosts esperados: PIDs `21812` e `34968` em A, e PID `37500` em B. Nenhuma
cópia global apareceu automaticamente. Os dois reinícios trocaram o PID e as
duas novas tentativas duplicadas preservaram a proprietária do endpoint.

### Edição e Salvamento sem o global

Com o status global indisponível, os quatro fluxos de edição, persistência e
releitura terminaram sem duplicata ou resultado ausente:

| Topologia | Projeto | Revisão | Bytes | SHA-256 | Status global |
| --- | --- | ---: | ---: | --- | --- |
| A | `project-spike-001` | 0 → 1 | 156.945 | `c0b42d77e5a8932eac56a23e4e0bd5398b9ac2e8e57ba71371f0ba6452d20f4d` | indisponível em 752,866 ms |
| A | `project-spike-002` | 0 → 1 | 155.870 | `096bb277689b6927e587ba0e6a164d23341eceefdd231231911a99175dd689f5` | indisponível em 755,938 ms |
| B | `project-spike-001` | 0 → 1 | 156.945 | `c0b42d77e5a8932eac56a23e4e0bd5398b9ac2e8e57ba71371f0ba6452d20f4d` | indisponível em 769,003 ms |
| B | `project-spike-002` | 0 → 1 | 155.870 | `096bb277689b6927e587ba0e6a164d23341eceefdd231231911a99175dd689f5` | indisponível em 764,619 ms |

Em todos os casos, a revisão relida foi `1`, a Sessão terminou
`dirty: false` e nenhum PID global foi reportado. Os hashes iguais entre as
topologias resultam da mesma intenção aplicada ao mesmo estado inicial, não do
compartilhamento de estado mutável.

Depois do reinício explícito, os dois Projetos avançaram de `1` para `2`,
foram relidos como revisão `2` e terminaram novamente limpos:

| Topologia | Projeto | PID global correlacionado | Status global | SHA-256 |
| --- | --- | ---: | ---: | --- |
| A | `project-spike-001` | `5932` | 0,974 ms | `cbfddf1e8be93f900cb7620420e5f42697665fa8fa226c0d943b01e492179950` |
| A | `project-spike-002` | `5932` | 1,692 ms | `ab4c81169ab0289899659050752d52f70c4ed708d2f211170f0c931ca74bf045` |
| B | `project-spike-001` | `29912` | 2,296 ms | `cbfddf1e8be93f900cb7620420e5f42697665fa8fa226c0d943b01e492179950` |
| B | `project-spike-002` | `29912` | 0,740 ms | `ab4c81169ab0289899659050752d52f70c4ed708d2f211170f0c931ca74bf045` |

Os tamanhos permaneceram 156.945 bytes para `project-spike-001` e 155.870
bytes para `project-spike-002`. Também nessa etapa houve duas conclusões por
topologia, zero duplicatas e zero ausências.

### Queda e reabertura dos hosts

As duas topologias mostraram o domínio de falha esperado:

| Evidência | A — hosts independentes | B — host multiwindow |
| --- | ---: | ---: |
| PID do host encerrado | `21812` | `37500` |
| Observação do término | 99 ms | 141 ms |
| Janelas restantes | 1 | 0 |
| Outro host sobreviveu | sim, PID `34968` | não se aplica |
| Reinício automático observado | não | não |
| PID do host reiniciado | `19652` | `29732` |
| Janelas prontas depois do reinício | 248 ms | 2.344 ms |
| Cache pronto depois do reinício | 5.738 ms | 4.940 ms |
| Duração de parede do Cache | 1.350 ms | 1.726 ms |
| Representações reutilizadas | 89 de 89 | 174 de 174 |
| Projetos reabertos | 1 | 2 |

Em A, o host sobrevivente avançou `project-spike-002` de revisão `2` para `3`,
persistiu 155.870 bytes com SHA-256
`6d21c4297ab1dc7a73203afea02d9d7ddd9a5d86d4c1f19c304187bb4f58fc16`
e releu a revisão `3`. A consulta correlacionou o processo global `5932` em
1,101 ms.

O host reiniciado de A reabriu `project-spike-001` na revisão explicitamente
salva `2`, sem tentar reconstruir uma edição posterior inexistente. Depois da
abertura, o global `5932` respondeu em 1,907 ms.

Em B, a queda do único host retirou conjuntamente as duas Janelas. Seu reinício
reabriu `project-spike-001` e `project-spike-002`, ambos na revisão `2`, no
mesmo PID `29732`. O processo global `29912` continuou proprietário e respondeu
em 3,116 ms.

### IPC observável

O gate registra relações e chamadas mínimas; ele não atribui um número
subjetivo de complexidade:

| Evidência | A — hosts independentes | B — host multiwindow |
| --- | ---: | ---: |
| Endpoints globais de escuta | 1 | 1 |
| Relações host → global | 2 | 1 |
| Relações interrompidas pela queda global | 2 | 1 |
| Probes de continuidade concluídos | 5 | 4 |
| Comandos mínimos ao host por probe | 4 | 4 |
| Comandos mínimos ao host | 20 | 16 |
| Interações mínimas por probe, incluindo status global | 5 | 5 |
| Interações mínimas correlacionadas | 25 | 20 |

Os quatro comandos ao host são configuração, leitura do estado, aplicação da
intenção e persistência. A quinta interação é o status tipado do host para o
global. Polls adicionais de configuração não entram na estimativa, portanto
as contagens são limites inferiores, não medições de tráfego total.

### Qualidade dos logs

| Evidência estruturada | A — hosts independentes | B — host multiwindow |
| --- | ---: | ---: |
| Streams do papel global | 4 | 4 |
| Eventos de início global | 2 | 2 |
| Rejeições de singleton | 2 | 2 |
| Eventos de status global | 10 | 9 |
| Campos globais obrigatórios ausentes | 0 | 0 |
| Streams dos hosts | 3 | 2 |
| Conclusões de continuidade | 5 | 4 |
| Falhas de continuidade | 0 | 0 |
| Eventos de reabertura | 1 | 2 |
| Campos de host obrigatórios ausentes | 0 | 0 |

Os eventos correlacionam `run_id`, topologia, papel, PID, Janela, Projeto e
probe quando aplicável. Tamanho, SHA-256, revisão anterior, revisão persistida,
estado `dirty`, disponibilidade global e latência também pertencem ao evento
de continuidade. As quatro terminações forçadas são observações do runner,
não mensagens atribuídas aos processos já encerrados.

### Processador de Imagens

O runner não repetiu mecanicamente as quedas do Processador. Ele exigiu o
artefato independente
[0004-imaging-recovery.json](artifacts/0004-imaging-recovery.json), schema
`1`, com fontes limpas e no mesmo commit
`65919f632feb32a53a22b16db3cbc551d82653f6`.

Seu SHA-256 foi
`65e9dc7fce21740fda906faa8e62f357dc46129f5ac94558f5e6b8f31bf9b210`.
Os quatro checks — protocolo, limpeza seletiva do Cache, build do sidecar e
prova integrada de produção — passaram. A evidência confirma recuperação do
Cache depois de um reinício explícito e falha segura da Exportação até uma
tentativa explícita. O contrato e o método dessas quedas estão descritos em
[Recuperação do Processador de Imagens](0010-recuperacao-do-processador-de-imagens.md).

O resultado agregado do schema `11` foi `failureGate.passed: true`.

## Limites da conclusão

Este gate não:

- implementa a UI real de Boas-vindas nem operações globais do produto. O
  papel global medido é intencionalmente headless; portanto o candidato
  completo a `MyAlbuns.exe` permanece em aberto;
- escolhe TCP loopback como IPC normativa;
- escolhe definitivamente entre as topologias A e B;
- cria eleição, watchdog, reinício automático ou coordenador universal;
- recupera alterações ainda não salvas, um gesto em andamento ou um
  checkpoint automático;
- transforma a persistência em `.scratch` no formato final do Projeto,
  `ProjectStore`, autosave ou `RecoveryStore`;
- demonstra recuperação de locks órfãos, `OperationGate`, `OperationLease` ou
  recursos de outro proprietário;
- prova limpeza de um Processador órfão depois da queda do próprio host. O
  runner apenas aguardou o Cache dos hosts reiniciados antes de encerrá-los;
- mede todo o tráfego de polling ou converte a IPC em um escore;
- usa uma única máquina e uma única ordem de execução como ranking de
  desempenho.

A recuperação comprovada termina na última revisão explicitamente salva. Essa
fronteira é deliberada: o gate não chama a sobrevivência de uma Janela ou de
um arquivo temporário de recuperação documental.

## Conclusão

Os critérios de falha e continuidade estão atendidos:

- nas duas topologias, as duas Janelas permaneceram abertas durante a queda do
  processo global e salvaram revisões verificadas enquanto as operações
  globais estavam indisponíveis;
- o global só voltou após ação explícita, com outro PID e singleton novamente
  comprovado;
- na topologia A, a queda de um host preservou a outra Janela, que continuou
  editando e salvando;
- na topologia B, a queda do host compartilhado retirou as duas Janelas;
- ambos os formatos de host reabriram exatamente as últimas revisões
  explicitamente salvas;
- a evidência do Processador de Imagens pertence ao mesmo commit e mantém as
  políticas distintas de Cache e Exportação;
- a IPC e os logs registraram todas as relações exigidas sem duplicatas,
  ausências ou falhas de continuidade.

Isso encerra a comparação exigida para essas falhas, mas não encerra o spike de
topologia. A UI real do processo global, o processo final de Boas-vindas, os
gates de caminhos e operações, a recomendação de topologia e a atualização do
ADR continuam separados.

## Repetição

A prova do Processador precisa preceder a rodada A/B e pertencer ao mesmo
commit com fontes limpas:

```powershell
npm run spike:imaging-recovery
npm run spike:topology
```

O segundo comando valida o primeiro artefato, reconstrói a build `release`,
executa as duas alternativas, escreve o schema `11` somente depois do sucesso
e remove os gates, processos e revisões descartáveis pertencentes à execução.
