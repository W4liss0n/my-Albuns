---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-28
updated: 2026-07-29
---

# Evidências do corte vertical da plataforma

## Resumo

O primeiro corte executável confirmou a viabilidade básica de Tauri 2, React,
TypeScript, Rust e PixiJS sobre WebGL2 para o MyAlbuns no computador de
desenvolvimento. O cenário abre um Projeto representativo, materializa parte de
um Álbum longo, aplica uma intenção documental no núcleo Rust, executa
Undo/Redo e exporta um `RenderSnapshot` por um processo auxiliar.

Esta é uma evidência intermediária do spike, não a sua conclusão. A comparação
entre as duas topologias, os ensaios com dois Projetos, Cache real, caminhos de
rede, injeção de quedas, instalador e máquina limpa continuam pendentes. O
ticket 01 não deve ser encerrado nem o ADR 0005 atualizado como decisão final
com base somente neste corte.

## Ambiente observado

| Item | Valor |
|---|---|
| Sistema alvo exercitado | Windows x64 |
| Node.js | `v24.18.0` |
| npm | `12.0.1` |
| Rust | `rustc 1.97.1` e `cargo 1.97.1` |
| Tauri CLI | `2.11.4` |
| WebView2 | `150.0.4078.99` |
| PixiJS | `8.19.0` |
| Backend gráfico | WebGL2 por ANGLE/D3D11 |
| Renderizador observado | `Intel(R) UHD Graphics (0x0000A78B)` |

O toolchain Rust está em `.tools/`, e os pacotes JavaScript estão em
`node_modules/`. Não foi feita instalação global de dependências. O Build Tools
2022 e o Windows SDK já existiam no computador e foram apenas descobertos pelos
scripts locais.

## Cenário executado

O Projeto de amostra, **Álbum Horizonte**, contém:

- 12 Lâminas lógicas de 60 × 30 cm;
- 22 Fotos inicialmente posicionadas;
- dois Frames placeholder na Lâmina 02;
- Frames, máscaras, Overlay, seleção e navegação contínua;
- uma única `ProjectSession` mutável mantida pelo backend Rust;
- estado transitório de seleção, foco e viewport mantido somente no frontend.

Na abertura, o Canvas materializou 5 das 12 Lâminas, preservando todas no
modelo lógico. A cena detalhada é reconstruída conforme o viewport, com margem
de pré-carregamento de uma Lâmina.

## Resultado funcional

O fluxo abaixo foi executado pela interface real:

1. abertura do Projeto na revisão 0;
2. duplo clique em uma Foto para preencher o Frame placeholder mais à esquerda;
3. decisão do Frame alvo pelo `ProjectCore`, sem ordenar os Frames placeholder no
   TypeScript;
4. avanço para revisão 1 e 23 Fotos posicionadas;
5. Undo para revisão 0 e 22 Fotos;
6. Redo para revisão 1 e 23 Fotos;
7. criação de um snapshot imutável;
8. envio do snapshot ao `myalbuns-imaging` por stdin;
9. exportação confirmada de PNG com 600 × 300 px.

O arquivo observado foi
`%LOCALAPPDATA%\Temp\MyAlbuns\spike\Album-Horizonte_001.png`, com 48.868 bytes.
O processo auxiliar usado no desenvolvimento tinha 1.148.928 bytes. Esses
tamanhos não representam o instalador ou os binários finais de produção.

## Fronteiras demonstradas

### Núcleo do Projeto

`ProjectCore` expõe uma sessão editável e uma carga de revisão persistida
somente para leitura. Os testes demonstram que a segunda forma não instancia
uma `ProjectSession` nem oferece comando, Salvamento ou Histórico.

`ProjectSession` é a única proprietária mutável do documento. Pan e Zoom da
Foto chegam como intenções consolidadas; a geometria intermediária do gesto
permanece na interface. A decisão do placeholder de menor borda esquerda é
executada no núcleo porque altera o documento e participa do Undo/Redo.

### Composição

`CompositionCore` produz um plano determinístico usado tanto pelo Canvas quanto
pela Exportação. `MediaTransform` pertence ao documento; o viewport do Canvas
fica fora do `RenderSnapshot`. Um teste específico impede que seleção ou
navegação sejam transportadas para o renderizador.

### Processo de imagem

O `myalbuns-imaging` aceita somente mensagens versionadas contendo um
`RenderSnapshot` imutável. Antes de produzir pixels, ele valida versão, unidade,
identificadores, dimensões, geometria, Pilha visual e transformações. Ele não
recebe caminho de arquivo de Projeto e não cria uma sessão editável. Para este
corte, uma instância curta é iniciada por Exportação e responde por stdout após
gravar o PNG.

### Superfície privilegiada

O frontend não recebeu permissão genérica de shell ou de sistema de arquivos.
Ele chama apenas comandos Tauri definidos pelo aplicativo. O backend inicia o
sidecar, e a capability da janela permanece limitada a `core:default`. CSP de
desenvolvimento e de produção estão declaradas separadamente.

## Verificações automatizadas

| Comando | Resultado em 2026-07-29 |
|---|---|
| `npm test` | 53 testes aprovados em 11 arquivos |
| `npm run test:rust` | 30 testes aprovados: 14 do núcleo, 3 do host, 5 do processador, 1 do protocolo, 4 da infraestrutura de logs e 3 de `AppPaths` |
| `npm run quality:rust` | `rustfmt` e Clippy aprovados sem avisos |
| `npm run build` | contratos, TypeScript e Vite aprovados; 2.022 módulos transformados |

Os testes de integração do processador iniciam o binário real, transmitem
mensagens pelo protocolo, validam a assinatura PNG, confirmam que Pan/Zoom
alteram os pixels finais e rejeitam um snapshot inválido antes da escrita.

O bundle web produziu uma entrada principal de 544,72 kB, ou 171,39 kB
comprimida, além dos chunks internos de renderização carregados pelo PixiJS. O
Vite emitiu aviso por ultrapassar 500 kB. Uma divisão manual anterior apenas
pré-carregava os mesmos módulos na abertura e foi removida por não reduzir o
trabalho inicial. Isso não bloqueia o corte, mas carregamento tardio do Canvas
deve ser medido antes de tratar o tamanho como aceitável.

## Falha encontrada e corrigida

Na primeira execução, a janela ficava vazia. O React em modo estrito desmontava
o efeito antes de `Application.init()` terminar, e a limpeza chamava
`Application.destroy()` cedo demais. O plugin de resize do PixiJS falhava com
`this._cancelResize is not a function`.

A integração agora só destrói uma aplicação PixiJS depois de sua inicialização
e garante uma única destruição. Um teste de regressão conclui fora de ordem as
duas inicializações produzidas pelo modo estrito e verifica que a montagem
abandonada não destrói a cena ativa.

## Observabilidade local

O host Tauri, o frontend e o Processador de Imagens agora emitem eventos
estruturados correlacionáveis. Os processos Rust gravam JSONL diário com
`process_role`, `protocol_version`, `operation_id` e IDs opacos quando
aplicáveis. O frontend usa um contrato estreito, encaminhado por um único
comando Tauri; mensagens livres, caminhos completos e conteúdo do Álbum não
fazem parte desse contrato.

IDs usados para correlação aceitam somente o formato opaco delimitado pela
infraestrutura. Valores em formato de caminho são omitidos, inclusive no
Processador de Imagens, e falhas de gravação não repetem o destino no stderr.
Cada Exportação produz um evento terminal `export_completed` ou `export_failed`
com a fase da falha.

Os arquivos ficam no namespace transitório
`%LOCALAPPDATA%\MyAlbuns2\Logs`, separados pelos prefixos
`myalbuns-desktop` e `myalbuns-imaging`, com retenção máxima de sete arquivos
por processo. A fila de escrita local não descarta eventos. `MYALBUNS_LOG`
permite ajustar o filtro para diagnóstico, e não existe telemetria remota.
`AppPaths` descobre as Known Folders uma vez em cada processo, e o host envia
ao Processador de Imagens o mesmo diretório exato. O sufixo `2` evita misturar
os dados desta geração com a versão anterior e será removido somente na
finalização do programa, conforme
[Armazenamento local e Cache](../design/0010-armazenamento-local-e-cache.md).

Uma abertura real registrou a sequência `application_started`,
`project_load_completed`, `canvas_initialization_completed` e
`canvas_scene_materialized`. No modo estrito, cada tentativa do PixiJS recebeu
um `instance_id`: o log distinguiu a montagem abandonada da que materializou a
cena. Esse encadeamento torna diretamente observável a classe de falha que
antes resultava apenas em uma janela vazia.

## Comandos de reprodução

```powershell
npm run setup:local
npm test
npm run test:rust
npm run quality:rust
npm run build
npm run tauri:dev
```

`setup:local` mantém downloads e instalações do projeto sob `.tools/` e
`node_modules/`. Os demais comandos reutilizam somente esse ambiente local.

## Limites desta evidência

Ainda faltam, no mínimo:

- executar o mesmo cenário nas topologias A e B;
- abrir e medir dois Projetos simultâneos;
- medir abertura, latência, memória de processo, memória gráfica, Cache e
  Exportação com massa representativa;
- exercitar perda e recuperação do contexto WebGL2 e limites de textura;
- integrar Boas-vindas e Configurações ao fluxo degradado sem liberar o editor;
- implementar e medir o Cache reduzido de mídias;
- injetar quedas do processo principal, host de Projeto e processador;
- validar `OperationGate`, `OperationLease`, pausa, cancelamento e liberação
  após falhas;
- ampliar a prova inicial de `AppPaths` com `RootBindingPlan`,
  `OperationPathContext` e identidades físicas em caminhos locais, UNC,
  unidade mapeada e caminhos verbatim;
- exportar com staging local e UNC;
- gerar o instalador `win-x64` e executar o teste em máquina limpa.

## Recomendação intermediária

Manter Tauri/React/Rust como direção do próximo incremento. O corte confirmou
as fronteiras mais importantes entre interface, estado canônico, composição e
processamento de imagem, além de validar WebGL2 real no ambiente atual. A
topologia de processos permanece aberta até que as medições e os testes de
isolamento exigidos pelo ticket sejam executados.
