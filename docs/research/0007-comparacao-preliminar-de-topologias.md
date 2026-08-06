---
status: historical
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

Esta é a primeira medição, hoje histórica, do esqueleto de hospedagem. Ela não
exercitou Pan/Zoom, Cache, Exportação, recuperação nem falhas do Processador de
Imagens. A rodada posterior com Cache e 172 Fotos reais está em
[0008-cache-com-imagens-reais.md](0008-cache-com-imagens-reais.md). Nenhuma das
duas escolhe uma topologia, encerra o ticket 01 ou muda o ADR 0005 para
`accepted`.

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

Na revisão que produziu esta primeira coleta, o Processador de Imagens era
iniciado somente para a Exportação de prova. O corte atual também o usa para
construir o Cache ao abrir o Projeto, mantendo uma instância e um namespace por
Projeto. A Exportação continua recebendo um snapshot imutável e não usa a
representação reduzida.

## Método

O relatório `0001` foi produzido por uma revisão anterior do instrumento, que:

1. preparava o sidecar e criava uma build debug com frontend embutido em um
   diretório isolado;
2. gravava um manifesto com commit, estado e digest das entradas, hash do
   executável e instante da build;
3. abria Horizonte e Aurora em dois hosts independentes;
4. esperava títulos que confirmassem a configuração de cada Janela;
5. somava host, WebView2 e demais processos descendentes observados;
6. coletava working set, memória privada, handles, threads e contadores de memória
   gráfica por PID;
7. forçava a queda de um host depois de validar que o PID pertencia ao executável
   do ensaio;
8. repetia a coleta com duas Janelas no host multiwindow;
9. gravava hardware, proveniência da build, dados brutos e um resumo Markdown
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

- o scaffolding privado serializa duas fixtures determinísticas e
  `ProjectCore::open_editable_session` abre ambas pela mesma entrada usada por
  um documento persistido; uma intenção aplicada ao primeiro Projeto não
  altera a revisão do segundo;
- `ProjectHost` encaminha a intenção pela Janela correta e preserva a sessão da
  outra Janela.

O ensaio real confirma pelos títulos e pelos logs que A e B carregam as duas
identidades. Os eventos `project_state_read` registram `window_label`,
`project_id` e revisão, permitindo distinguir as sessões sem registrar conteúdo
criativo.

## Repetição

A instrumentação atual supersede esta coleta: usa build `release`, valida
também o hash do Processador de Imagens e gera o relatório `0002` com o corpus
real:

```powershell
npm run spike:topology
```

Nova coleta usando a build isolada já existente:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Measure-TopologySpike.ps1 -SkipBuild
```

O modo rápido exige o manifesto criado pela build, verifica novamente os hashes
do host e do Processador de Imagens e atribui a coleta ao commit gravado nesse
manifesto, não ao `HEAD` que estiver selecionado depois.

Cada execução atual substitui o JSON `0002` e seu resumo derivado. Os artefatos
`0001` preservam esta primeira coleta e não são mais sobrescritos. A aceitação
final deve usar uma build cujas entradas estejam limpas, alternar a ordem das
alternativas e registrar amostras suficientes para reduzir o efeito de
aquecimento.

## Próximo gate

A próxima rodada deve automatizar exatamente as mesmas operações de Pan/Zoom e
Exportação nos dois Projetos, registrar prontidão real do Canvas, medir
latências e o ciclo de vida dos Processadores de Imagens e repetir as amostras
alternando a ordem. Depois disso entram recuperação e as demais injeções de
falha previstas no ticket 01.
