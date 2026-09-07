---
status: current
document: research
date: 2026-09-06
platform: windows
---

# Retenção de prévias ao rolar o Painel

O retorno às primeiras linhas deixava espaços vazios porque sair da demanda
visível revogava tanto a URL opaca quanto os bytes da prévia. A resposta final
do Host continha somente a demanda nova; `App` substituía seu snapshot e retirava
os elementos de imagem anteriores. Ao voltar, a publicação passava novamente
pelo preparo de Cache. O Painel já conservava seus cards no DOM: o problema
estava na política de residência, e não na virtualização das Lâminas no Canvas.

## Evidência e decisão

- Admission route: `production-reachable`.
- Reachability or current consumers: observadores do `MediaPanel` → demanda do
  `ProjectWorkspace` → `prepare_media_previews` → `CacheEngine` →
  `CachePreviewRegistry`; a conclusão substitui `mediaPreviews` em `App`.
- Evidence: o teste nativo de sair/voltar retornou `None` para uma prévia
  publicada; o teste da interface retirou o `img` depois da conclusão da demanda
  inferior. Ambos foram executados antes da correção.
- Current impact: miniaturas já vistas voltavam à fila e apareciam aos poucos.
- Owning module: `CachePreviewRegistry`.
- Recommended change: separar a residência de prévias prontas do cancelamento
  de trabalhos obsoletos, conservando URLs recentes em um cache limitado.
- Verification boundary: publicação nativa, retorno do snapshot e identidade
  do elemento de imagem no `App` real.

O registro nativo passou a ser o único dono da retenção recente. A conclusão
inclui as prévias ainda residentes; estados explícitos de falha prevalecem sobre
contexto antigo. A interface continua substituindo o snapshot, de modo que uma
evicção também retire a imagem correspondente. Não foi criado outro cache de
URLs no React. O Canvas continua sincronizando somente as texturas das Lâminas
visíveis e de sua margem.

Os limites aceitos em [Armazenamento local e Cache](../design/0010-armazenamento-local-e-cache.md)
são 64 MiB codificados, 512 entradas e 2 GiB na estimativa de apresentação, fora
da demanda ativa. O descarte segue o uso menos recente. A estimativa inicial de
1 GiB seria insuficiente para conservar este conjunto: as 134 representações
somam 46,70 MiB codificados e 1.008,44 MiB na estimativa RGBA, antes de somar os
bytes codificados. O limite de 2 GiB não significa reservar essa quantidade de
RAM. O navegador pode recuperar memória de imagens sob pressão; a garantia é
reusar uma prévia ainda residente, sem prometer residência ilimitada.

Retenção sem limite não atende a catálogos grandes. Uma segunda representação
menor para o Painel reduziria a memória estimada, mas acrescentaria preparo,
publicação e invalidação de outra variante. A medição atual não exige esse
custo: a representação única já apresentou as imagens de volta no primeiro
quadro observado. Essa alternativa pode ser reavaliada se houver pressão de
memória medida em catálogos maiores.

## Verificação nativa com as 134 fotos

O ensaio `photo_import::native_flow_tests::real_import_flow` usa Host, núcleo,
Monitor, Cache e Processador reais, com os JPEGs autorizados. Executado em
Debug, sem outros builds ou medições concorrentes:

| Resultado | Medição |
|---|---:|
| Fotos importadas / prévias publicadas | 134 / 134 |
| Preparo inicial e importação | 6.421,02 ms |
| Primeira entrega de todas as prévias | 262,36 ms |
| Percorrer demandas, esvaziá-las e retornar | 12,24 ms |
| URLs reutilizadas depois do percurso | 134 / 134 |
| Novos trabalhos de preparo durante esse retorno | 0 |

O retorno nativo mede reconciliação e consulta de residência, não pintura de
tela. Os hashes dos Originais permanecem iguais, e a importação continua sendo
uma ação de Histórico. Evidência: [native-134.json](D:/CodexBuilds/myalbuns-panel-residency/native-134.json).

## Navegador e apresentação

O ensaio separado executou o `App`, o Painel, os observadores e os elementos de
imagem reais no Edge 152.0.4191.62, com 134 JPEGs reduzidos do Cache. Apenas a
resposta da porta nativa foi simulada para comparar os contratos antigo e novo.
Depois de percorrer todas as linhas usando eventos de roda do WebDriver, novas
respostas foram bloqueadas antes de voltar ao topo.

| Ao voltar, antes de qualquer nova resposta | Contrato antigo | Com retenção |
|---|---:|---:|
| Cards visíveis | 18 | 18 |
| Imagens completas no primeiro `requestAnimationFrame` | 0 | 18 |
| Mesmos elementos `img` da primeira visita | 0 | 18 |
| Novas requisições de JPEG durante o retorno | 0 | 0 |

Na execução com retenção, o callback do primeiro quadro ocorreu 0,50 ms depois
do evento de rolagem. Esse intervalo não é uma medição da pintura física da
tela. A captura confirma as miniaturas renderizadas, com a porta ainda bloqueada.
O processo do WebDriver e seus sete processos filhos somaram 381,35 MiB de
memória privada depois do percurso. Isso inclui o navegador inteiro, não o Host
nativo; o ensaio usa o mesmo navegador para as duas políticas e não constitui
uma comparação isolada de consumo. A soma dos working sets não é tratada como
RAM exclusiva, pois pode contar páginas compartilhadas mais de uma vez.

Evidências privadas locais: [resultado](D:/CodexBuilds/myalbuns-panel-residency/browser/results.json),
[retorno corrigido](D:/CodexBuilds/myalbuns-panel-residency/browser/resident-return.png)
e [retorno com o contrato antigo](D:/CodexBuilds/myalbuns-panel-residency/browser/old-return.png).
O ensaio não substitui a confirmação na Janela nativa do usuário.

Os contratos externos consultados foram React 19.2 (instalado 19.2.8) para a
preservação de elementos e o [protocolo WebDriver](https://www.w3.org/TR/2026/WD-webdriver2-20260702/)
para rolagem e execução de scripts no Edge 152, com driver da mesma versão.

## Regressões cobertas

Os testes cobrem os três limites de retenção, proteção da demanda ativa,
descarte por uso, revogação do token descartado, snapshot com falha explícita,
alteração da origem, remoção, Religação e retirada de Identidade. A barreira de
publicação conserva trabalhos concluídos antes da troca de demanda; trabalhos
obsoletos continuam impedidos de publicar depois da troca.

A suíte Rust passou com 654 testes e 18 cenários reservados para execução
específica. Após o ajuste do limite e do estilo, os 82 testes relacionados ao
Cache, o ensaio com 134 fotos e `fmt`/`clippy` passaram novamente. Os 97 testes
relacionados de interface e porta Tauri também passaram.

A prevenção é manter separados o ciclo de vida de um trabalho pendente e o de
uma prévia já publicada, testando sair e voltar pela mesma fronteira usada em
produção. A skill de diagnóstico foi seguida com testes vermelhos antes da
correção; não restou instrumentação temporária no código do produto.
