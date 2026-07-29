---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-29
updated: 2026-07-29
---

# Comparação preliminar das topologias de Projeto

## Resumo

O primeiro esqueleto executável das duas topologias abriu simultaneamente os
Projetos de amostra **Álbum Horizonte** e **Álbum Aurora**:

- na alternativa A, cada Projeto pertenceu a um processo host e a uma
  `ProjectSession` próprios;
- na alternativa B, um processo host manteve duas Janelas, cada uma ligada a
  uma `ProjectSession` distinta;
- alterar a sessão de uma Janela não alterou a revisão da outra nos testes;
- a queda forçada de um host de A preservou a outra Janela;
- a queda do host de B encerrou as duas Janelas.

Esta é uma medição intermediária do esqueleto de hospedagem. Ela ainda não
exercita o cenário completo de Pan/Zoom, Cache, Exportação, recuperação e
falhas do Processador de Imagens. Portanto, não escolhe uma topologia, não
encerra o ticket 01 e não muda o ADR 0005 para `accepted`.

Os dados brutos estão em
[0001-topology-spike-baseline.json](artifacts/0001-topology-spike-baseline.json).
O resumo numérico, gerado da mesma estrutura em cada execução, está em
[0001-topology-spike-baseline.md](artifacts/0001-topology-spike-baseline.md).

## Corte implementado

O comportamento normal do aplicativo permanece com uma Janela e um Projeto. Os
modos comparativos são habilitados somente por variáveis explicitamente
reservadas ao spike.

`ProjectHost` mantém um mapa imutável entre o rótulo da Janela e a sessão. Cada
entrada possui seu próprio `Mutex<ProjectSession>`; não existe um bloqueio
mutável compartilhado entre os Projetos. Os comandos Tauri recebem a
`WebviewWindow` chamadora e resolvem a sessão por seu rótulo:

- A cria dois processos do mesmo executável, cada qual com a Janela `main` e
  apenas uma sessão;
- B cria um processo com as Janelas `main` e `project-b` e duas sessões;
- ambas usam o mesmo `ProjectCore`, o mesmo frontend e as mesmas cenas de 12
  Lâminas;
- os Projetos têm identidades distintas, `project-spike-001` e
  `project-spike-002`;
- a capability continua restrita a `core:default` e aos dois rótulos conhecidos,
  sem acesso genérico ao shell ou ao sistema de arquivos.

O Processador de Imagens ainda é iniciado somente quando a Exportação de prova
é solicitada. O isolamento e o ciclo de vida desse processo em cada topologia
continuam pendentes.

## Método

O comando `npm run spike:topology`:

1. prepara o sidecar e cria uma build debug com frontend embutido em um
   diretório isolado;
2. abre Horizonte e Aurora em dois hosts independentes;
3. espera títulos que confirmem a configuração de cada Janela;
4. soma host, WebView2 e demais processos descendentes observados;
5. coleta working set, memória privada, handles, threads e contadores de memória
   gráfica por PID;
6. força a queda de um host depois de validar que o PID pertence ao executável
   do ensaio;
7. repete a coleta com duas Janelas no host multiwindow;
8. grava hardware, commit, estado da árvore, dados brutos e um resumo Markdown
   derivado da mesma estrutura.

Os hosts de A são iniciados sequencialmente. Em uma tentativa de início
simultâneo, um dos dois processos permaneceu vivo sem apresentar Janela dentro
de 45 segundos. A condição foi reproduzida uma vez em três tentativas desta
rodada e deve ser investigada no launcher e no isolamento do ambiente WebView2
antes da aceitação da alternativa A. O relatório não descarta esse resultado.

A memória foi amostrada 750 ms depois de as Janelas esperadas aparecerem. O
tempo de abertura abaixo termina quando os títulos esperados estão visíveis;
ele não representa prontidão completa do Canvas. A ordem A e depois B aquece
arquivos e runtimes para a segunda alternativa, portanto os tempos servem
somente para validar o instrumento nesta rodada.

## Ambiente

Sistema, processador, memória física, adaptadores gráficos, commit e perfil da
build são registrados pelo instrumento. Os valores da última execução ficam no
[resumo gerado](artifacts/0001-topology-spike-baseline.md), evitando uma segunda
cópia manual que possa divergir dos dados brutos.

O contador registrou somente memória gráfica compartilhada e zero bytes
dedicados para as árvores observadas. Esse resultado não comprova por si só o
backend WebGL2 nem a aceleração por hardware; a verificação gráfica específica
do ticket continua necessária.

## Resultado bruto resumido

O [resumo gerado](artifacts/0001-topology-spike-baseline.md) é a única tabela
numérica legível desta coleta e nasce do mesmo objeto que produz o JSON. Uma
única amostra debug, sem estabilização prolongada e com ordem fixa não sustenta
decisão de arquitetura.

O resultado de falha é estrutural e já distingue as alternativas: A limita a
queda ao Projeto cujo host foi encerrado; B compartilha o domínio de falha das
duas Janelas. Ainda faltam recuperação persistida, continuidade de Salvamento,
queda do processo global e queda do Processador de Imagens.

## Evidências automatizadas

Os testes adicionados verificam duas fronteiras:

- `ProjectCore::open_sample_project` recebe uma fixture tipada, abre identidades
  distintas e uma intenção aplicada ao primeiro Projeto não altera a revisão do
  segundo;
- `ProjectHost` encaminha a intenção pela Janela correta e preserva a sessão da
  outra Janela.

O ensaio real confirma pelos títulos e pelos logs que A e B carregam as duas
identidades. Os eventos `project_state_read` registram `window_label`,
`project_id` e revisão, permitindo distinguir as sessões sem registrar conteúdo
criativo.

## Repetição

Build e medição completas:

```powershell
npm run spike:topology
```

Nova coleta usando a build isolada já existente:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Measure-TopologySpike.ps1 -SkipBuild
```

Cada execução substitui o JSON e seu resumo derivado. A aceitação final deve
usar uma build cujas entradas estejam limpas, repetir a ordem das alternativas
e registrar amostras suficientes para reduzir o efeito de aquecimento.

## Próximo gate

A próxima rodada deve automatizar exatamente as mesmas operações de Pan/Zoom e
Exportação nos dois Projetos, registrar prontidão real do Canvas, medir
latências e Processadores de Imagens e repetir as amostras alternando a ordem.
Depois disso entram Cache, recuperação e as demais injeções de falha previstas
no ticket 01.
