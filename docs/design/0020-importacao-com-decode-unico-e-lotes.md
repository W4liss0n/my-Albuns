---
status: accepted
document: design
date: 2026-09-06
platform: windows
implementation-readiness: ready-for-agent
---

# Importação com decode único e lotes de processamento

Este contrato detalha a integração autorizada das otimizações de importação.
Preserva a ação única de Histórico, a ordem da seleção, o resultado parcial,
a espera de Salvar/Fechar e a apresentação conjunta dos novos cartões.

## Tentativa e inspeção

O Host cria uma tentativa limitada ao processamento de imagens. Ela obtém uma
vez o catálogo autorizado, captura as raízes necessárias em um único
`RootBindingPlan` e identifica cada candidato por uma chave de fonte temporária.
A chave de fonte não é um `MediaId` nem entra no Projeto ou no índice do Cache.
Todos os participantes reutilizam o plano, inclusive a inspeção alternativa.

O Processador decodifica cada JPEG aceito pelo percurso normal uma única vez.
O mesmo raster fornece dimensões orientadas e a representação reduzida. Apenas
metadados e resultados tipados atravessam IPC; pixels do Original ficam no filho.
O resultado distingue Original validado, inspeção alternativa necessária e
falha de Cache. Um erro de escrita/encode da prévia não invalida o Original que
já foi decodificado e teve seu fingerprint confirmado.

Quando o Processador não pode concluir a inspeção, o Host usa a fronteira de
validação existente sob o mesmo orçamento de capacidade e o plano capturado.
Esse percurso excepcional preserva os JPEGs válidos que a política de Cache
recusa. Não suprime corrupção nem autoriza uma fonte ausente por sua prévia.

O Host reconfirma observações e Identidade antes do commit. O Core atribui os
`MediaId` definitivos. O Monitor recebe uma operação explícita para adotar
evidências comprovadas; os consumidores não conhecem o número de amostras usado
para estabilizar mudanças externas.

## Gerações antes do commit

As prévias candidatas ficam na pasta `media` do namespace, com nome opaco
derivado da tentativa e da chave de fonte. Não há índice provisório com mídias
fictícias, outra pasta persistente ou cópia de Original. Cada candidato é
protegido pela atividade da tentativa, além do registro durável do escritor.

Depois do commit, o `CacheEngine` confirma os bytes e o vínculo, renomeia cada
geração para o dono definitivo e publica as entradas em uma transação do índice.
Uma falha conserva os vínculos válidos e informa indisponibilidade do Cache.
Cancelamento ou retirada de Identidade descarta candidatos; recuperação após
queda espera os escritores e recolhe as gerações sem entrada no índice.

## Processadores e índice

Um processo recebe um lote limitado e processa uma imagem por vez. O limite de
32 evita iniciar um processo por foto e mantém a contenção existente: handshake,
Job do Windows, registro durável antes do primeiro item, cancelamento e saída
confirmada antes de liberar a reserva. Não existe um segundo controlador de
processos paralelo ao `ImagingProcessor`.

`ImagingProcessor` possui os limites de trabalhadores e de memória para todas
as ações e demandas de Cache. A política considera capacidade de CPU, RAM física
e commit disponíveis, limita reservas por imagem e espera de forma cancelável
sob pressão. Não aprende continuamente pela velocidade do lote. A Exportação
continua pausando Cache e adquirindo exclusividade de toda a capacidade.
As inspeções do Monitor e a inspeção alternativa da importação usam essa mesma
reserva, liberam esperas quando há pausa e drenam um decoder iniciado antes de
devolver recursos. A inspeção de Religação reserva capacidade dentro da pausa
exclusiva que a ação já possui, sem adquirir atividade de Cache aninhada.

O teto é de oito trabalhadores, limitado à capacidade lógica de CPU menos um,
com mínimo de um. A estimativa por Original considera 16 bytes por pixel,
duas vezes o tamanho comprimido e 64 MiB para buffers e processo. Um lote
reserva o máximo entre suas imagens, pois só mantém um Original por vez.
Essa estimativa de admissão complementa os limites do codec; não constitui uma
garantia contra mudanças de memória feitas por outros programas após a leitura.

O orçamento agregado por Host usa no máximo um quarto da RAM total e 4 GiB.
A admissão também observa metade da RAM disponível e do commit disponível,
depois de preservar uma margem de um oitavo da RAM total, limitada entre
512 MiB e 2 GiB. A cada 100 ms, a espera reavalia pressão externa, cancelamento
e quarentena. Ausência de telemetria permite apenas uma reserva até 1 GiB;
uma imagem acima do teto produz falha explícita de recursos.

`CacheEngine` possui a interface transacional do índice: consulta por mídia em
uma leitura validada da operação, agrupamento de alterações e publicação
atômica. O planejamento não desserializa o catálogo por foto. Nenhum snapshot
em memória é reutilizado entre operações apenas por tamanho ou data do arquivo.
A entrega de prévias e o commit observam o índice atual; corrupção, ausência e
substituição continuam impedindo o reaproveitamento de evidência antiga.

Uma ação comum prepara suas gerações, publica as entradas em conjunto e só
então conclui os trabalhos compartilhados. Os consumidores que aguardam uma
geração não recebem sucesso antes do índice. A primeira demanda consulta em
conjunto os recibos de preparação; uma demanda que precise gerar uma nova
prévia continua podendo publicá-la individualmente, conforme sua conclusão.
Essa publicação individual conserva a entrega progressiva do Painel e Canvas.

O `CacheEngine` também concentra a tentativa única de recuperação de queda e
a suspensão após reincidência, tanto para importação quanto para outros
trabalhos de Cache. Sem confirmação de saída de um escritor, a limpeza de
gerações fica bloqueada até a recuperação por uma nova instância do Host.

Host e Processador compartilham a política de validação das representações
reduzidas. Cada um conserva sua própria abertura autorizada e decisão local:
reconstrução no Processador, rejeição e digest no Host.

## Verificação

- Equivalência de pixels, bytes, dimensões e orientação para prévias RGB/RGBA.
- Arquivo válido com Cache indisponível, JPEG inválido, falha parcial e duplicatas.
- Identidade/fonte alterada e raízes remapeadas entre fases.
- Cancelamento ativo ou aguardando recursos, queda do filho e queda do Host.
- Registros de todos os escritores, retomada e exclusividade da Exportação.
- Índice ausente/corrompido/substituído, publicação concorrente e lotes grandes.
- Importação pela UI, Monitor concorrente, Salvar/Fechar pendentes e cartões juntos.

`scripts/Test-Rust.ps1` inclui um ensaio com o executável real, desde os lotes
nativos até o commit no Core, a adoção no Monitor e o atendimento das prévias.
Ele cobre JPEG progressivo, corrupção, formato incompatível e perfil de cor
que exige inspeção alternativa. Esse ensaio não abre WebView nem seletor
nativo; a validação dessas superfícies permanece separada.

## Contratos externos consultados

Versões fixadas: Rust 1.98.0, `image` 0.25.10, `tokio` 1.53.1,
`serde_json` 1.0.151, Tauri 2.11.5 e `windows-sys` 0.61.2.
`find-docs` foi usado antes das decisões. A documentação indexada de `image`
inclui APIs posteriores; o contrato aplicável foi confirmado no código local
da versão 0.25.10, que oferece `ImageReader::into_decoder` e mantém o tipo de
pixel em `DynamicImage::thumbnail`. A documentação oficial de
[BufReader](https://doc.rust-lang.org/std/io/struct.BufReader.html) sustenta a
leitura bufferizada; a de [Child](https://doc.rust-lang.org/std/process/struct.Child.html)
fundamenta a distinção entre solicitar encerramento e confirmar a saída.
O contrato de [PERFORMANCE_INFORMATION](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-performance_information)
define RAM e commit em páginas; o Host multiplica por `PageSize` e calcula
commit disponível por `CommitLimit - CommitTotal`. O binding foi confirmado
no código de `windows-sys` 0.61.2. A semântica de
[Notify](https://docs.rs/tokio/1.53.1/tokio/sync/struct.Notify.html) e a reserva
de semáforos foram consultadas por `find-docs` e confirmadas no código local
de Tokio 1.53.1 para não perder notificações ao liberar memória.
