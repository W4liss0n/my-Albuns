---
status: proposed
document: design
date: 2026-09-29
---

# Acesso aos Arquivos vinculados

## Objetivo

Todo acesso do Host a Arquivos vinculados e às pastas de onde eles vêm passa por um
único módulo, `linked_files` (`src-tauri/src/linked_files.rs`). Os fluxos pedem o que
precisam, em lote, e **nenhum fluxo decide sozinho se um arquivo está em disco local
ou num compartilhamento de rede**.

Numa pasta de rede, cada acesso espera várias idas e voltas ao servidor e cada byte
atravessa a rede. Nas medições locais de 2026-09-29, em Wi‑Fi:

- observar um Original custava de 48 a 50 ms um por vez e de 13 a 15 ms com oito ao
  mesmo tempo;
- o cliente SMB guardou em memória arquivos lidos pouco antes, mas isso não é
  garantido: com o cache frio, toda releitura baixa o arquivo de novo.

A pesquisa [Desempenho com Projetos e imagens em rede](../research/2026-09-29-desempenho-em-rede.md)
registra as demais medições.

As regras que tornam isso aceitável ficam no módulo, e corrigir uma delas corrige
todos os fluxos. Antes da centralização, a inspeção de cabeçalho deixou de ler o corpo
do JPEG, mas a estimativa de memória, implementada em outro lugar, continuou lendo o
arquivo inteiro.

## Operações

| Operação | O que devolve | Lê |
| --- | --- | --- |
| `observe` / `observe_unless` / `observe_one` / `observe_in_plans` | identidade física, tamanho, datas e se o conteúdo é legível | abre o arquivo e lê 1 byte |
| `headers` / `header` | formato, dimensões já orientadas, se é JPEG sequencial colorido e tamanho | JPEG só até o cabeçalho do primeiro scan; PNG e TIFF até os cabeçalhos do decoder |
| `inspect_decoded` | o mesmo cabeçalho, depois de decodificar a imagem inteira | o arquivo inteiro |
| `list_folder` / `list_folders` | entradas da pasta com tipo, tamanho e datas | a listagem; só um link é seguido com mais uma consulta |

Todas recebem o `RootBindingPlan` da tentativa, devolvem na ordem pedida e bloqueiam;
os chamadores rodam fora da thread da interface. `observe_unless` responde logo
depois que uma pausa do Cache é pedida, sem esperar as observações em andamento:
elas terminam sozinhas e o resultado é descartado. O Monitor lista as pastas antes
de segurar a permissão do Cache, porque a listagem não pode ser interrompida.

Um único cabeçalho atende aos dois usos: `SourceHeader::photo_metadata` produz os
metadados da Foto e `ImageMemoryEstimate::from_headers` produz a estimativa de
memória, sem outra leitura.

## O que fica dentro do módulo

- **Concorrência**: até oito acessos ao mesmo tempo, com a ordem do resultado
  preservada. Nenhum chamador escolhe o número.
- **Cabeçalho sem corpo**: `myalbuns_imaging::source_header::jpeg_header_prefix`, com
  o mesmo parser disponível ao Processador.
- **Listagem sem consulta extra por entrada**: atributos, tamanho e datas vêm da
  própria enumeração do Windows.
- **Medição**: o evento `linked_files_batch_completed` registra operação, se a raiz é
  remota, quantidade e duração, sem caminhos. Vai para o log de produção a partir de
  1 s e para `debug` abaixo disso.
- **Raiz local ou remota**: `RootBindingPlan::is_remote` em `myalbuns-paths`. Uma raiz
  UNC é remota, e uma unidade mapeada já chega resolvida para a raiz UNC. Um disco
  que o Windows informa como remoto também é remoto.
- **Poucas leituras completas na rede**: no máximo três leituras completas de
  Originais remotos ao mesmo tempo, somando lotes de importação e jobs de Cache
  (`full_read_capacity` e `remote_read_turn`). Os turnos seguem a ordem dos pedidos,
  e um pedido que ficou obsoleto desiste da vez. Em disco local nada muda. Na
  medição de 2026-09-29, a vazão total era a mesma com 1, 2 ou 8 leituras, mas com
  8 a primeira foto chegava depois de 1,47 s, contra 0,31 s com uma.
- **Servidor que parou de responder**: o Windows espera cerca de 37 s por um
  servidor desligado e guarda a falha só por 20 a 30 s. Quando um acesso sob uma
  raiz remota falha por rede ou servidor (`ResolveError::Unavailable`), a raiz fica
  marcada, e todo acesso a ela falha na hora até uma sonda própria, a cada 2 s,
  encontrar o servidor de novo. Um arquivo aberto em outro programa não marca a
  raiz, e raízes locais nunca são marcadas.
- **Adaptadores de teste**: `LinkedFiles::with_latency` simula um compartilhamento
  lento nos testes de paralelismo e de interrupção pela pausa do Cache;
  `with_unreachable_roots` decide quando um servidor volta a responder.

Um teste de arquitetura (`linked_files/architecture_tests.rs`) falha quando código de
produção fora do módulo abre ou decodifica imagens de arquivo, lista pastas com uma
consulta por entrada, estima memória lendo Originais ou observa e inspeciona
Originais por conta própria. Cada exceção fica listada com o motivo.

## Mapa dos fluxos

| Fluxo | observe | headers | inspect_decoded | list_folder |
| --- | :-: | :-: | :-: | :-: |
| Abertura: recuperação do Cache e preparação das imagens | ✓ | ✓ | | |
| Monitor de Arquivos vinculados | ✓ (interrompível) | | ✓ (mudança confirmada) | ✓ |
| Importação de imagens e de pastas | ✓ | ✓ | ✓ (verificação da seleção) | ✓ |
| Religação e Substituir imagem | ✓ | ✓ | ✓ | ✓ |
| Preparação do Cache e publicação das prévias | ✓ | ✓ | | |
| Exportação normal (verificação prévia) | ✓ | | | |
| Exportação em lote (planejamento e descoberta de projetos) | ✓ | ✓ | | ✓ |
| Geração em lote (pastas de origem) | | | | ✓ |
| Visualizador | | | ✓ | |

## Fora do módulo

- **Processador**: lê os Originais no próprio processo, sob o `RootBindingPlan`
  congelado. Compartilha só as funções puras de `myalbuns-imaging`.
- **Correção de olhos e Photoshop**: substituem o Original com regras próprias de
  backup.
- **Arquivo do Projeto, stores locais, Cache e Recuperação**: não são Arquivos
  vinculados.
- **Prévias em memória**: decodificam bytes já lidos, não acessam arquivos.

## Pendências

- **Falha rápida de raiz inacessível**: consultar a raiz uma vez e marcar todas as
  mídias dela como indisponíveis, sem tentar arquivo por arquivo. Fica para depois de
  medir o comportamento com o servidor fora do ar (P8 do plano de desempenho).
- **Medição ponta a ponta** do limite de leituras remotas (P0 do plano de
  desempenho): o número três vem das medições por componente.
