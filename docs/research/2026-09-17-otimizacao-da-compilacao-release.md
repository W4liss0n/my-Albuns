---
status: current
document: research
date: 2026-09-17
platform: windows-x64
---

# Otimização da compilação Release

O objetivo solicitado foi reduzir o tamanho da distribuição e acelerar o
programa. A referência é uma compilação Release do commit
`0b81f146e0e8cafdca621f8bbf9602d9b3458b40`, não a compilação Debug usada no
desenvolvimento. Aplicativo, Processador e instalador foram reconstruídos nas
duas variantes.

## Configuração

O perfil Release da workspace usa `lto = "thin"` e `codegen-units = 1`.
`opt-level = 3` continua sendo o padrão do Cargo. A configuração explicita
`panic = "unwind"` porque o trabalhador de JPEG progressivo usa `catch_unwind`
para converter falhas em respostas controladas.

O Tauri usa `build.removeUnusedCommands = true`. As permissões são estáticas;
os 46 comandos globais e 70 comandos de Projeto permanecem cobertos. Chamadas
nativas de diálogo e execução do Processador usam as APIs Rust. Não há
registro dinâmico de capacidades nem chamadas de plugins pelo frontend.

Foram conferidas as versões Rust/Cargo 1.98.0, Tauri 2.11.5, tauri-build 2.6.3
e CLI 2.11.4. O Context7 estava sem cota; a consulta usou a documentação
oficial de [perfis do Cargo](https://doc.rust-lang.org/cargo/reference/profiles.html)
e [tamanho de aplicações Tauri](https://v2.tauri.app/concept/size/).
ThinLTO permite otimização entre crates; reduzir as unidades de geração de
código pode melhorar o resultado, com maior custo de compilação. A remoção
de comandos depende das permissões estáticas e deve ser reavaliada caso o
produto passe a adicionar capacidades dinamicamente.

## Método

Máquina local Windows x64, Intel Core i5-13450HX, 10 núcleos e 16 processadores
lógicos. As duas distribuições usam o mesmo código funcional, dependências e
frontend. Os executáveis de referência foram preservados antes de aplicar as
opções novas.

O [script do experimento](artifacts/2026-09-17-compare-release.mjs) executa os
dois Processadores reais pelo protocolo 26. Usa cópias de cinco JPEGs reais,
totalizando 25.037.650 bytes; os Originais não são modificados. Cada cenário
tem duas rodadas de aquecimento e oito amostras válidas por variante, com
ordem alternada. As medições finais ocorrem sem compilação concorrente.
O executor do experimento usa Node.js 24.18.0. As fotos permanecem locais;
o relatório versionado identifica o corpus pelos hashes. Outros arquivos
podem produzir tempos diferentes.

- Inicialização: processo do Processador e resposta a `--protocol-version`.
- Importação: inspeção e preparação de cinco prévias novas, sem reutilização
  de gerações anteriores.
- Exportação: composição de cinco imagens decorativas numa Lâmina sintética
  de 600 × 300 mm e saída JPEG a 300 dpi.

O cronômetro inclui criação e encerramento do processo. A leitura dos hashes
de saída fica fora do intervalo medido. As prévias e os JPEGs precisam ser
idênticos byte a byte entre todas as execuções. A inicialização gráfica, a
interação com janelas e a exportação em PNG/PDF não são medidas por este
experimento. Os resultados não representam todos os projetos ou computadores.

## Resultados

Os tamanhos usam MB decimais (1 MB = 1.000.000 bytes):

As [amostras e os hashes dos artefatos](artifacts/2026-09-17-release-comparison.json)
registram os valores completos e a configuração medida.

| Artefato | Release anterior | Release otimizada | Redução |
| --- | ---: | ---: | ---: |
| Aplicativo | 28,95 MB | 21,66 MB | 25,2% |
| Processador de Imagens | 3,93 MB | 3,74 MB | 5,0% |
| Instalador NSIS | 7,46 MB | 6,38 MB | 14,4% |

| Cenário | Mediana anterior | Mediana otimizada | Variação do tempo |
| --- | ---: | ---: | ---: |
| Inicialização do Processador e versão | 20,28 ms | 23,19 ms | +2,91 ms |
| Preparação de cinco prévias | 989,47 ms | 985,85 ms | −0,4% |
| Exportação JPEG | 2.037,34 ms | 2.008,10 ms | −1,4% |

As 100 prévias e 20 exportações das duas variantes, incluindo aquecimento,
mantiveram os mesmos hashes. O benefício confirmado é a redução de tamanho.
As pequenas diferenças no processamento não sustentam uma promessa de ganho
geral de velocidade; a consulta de versão ficou 2,91 ms mais lenta neste
ensaio. Não foi medida a abertura da interface gráfica. O teste preliminar
com o mesmo executável dos dois lados serviu apenas para validar o script e
foi excluído desses resultados.

A geração completa observada levou 453 s na referência e 813 s na versão
otimizada. Esses tempos incluem preparação do Processador, frontend e
instalador, com caches de compilação diferentes; não constituem um benchmark
controlado do tempo de build.

## Validação

`npm run validate` passou nas sete etapas: preparação do Processador, build
do frontend (contratos e tipos), dimensionamento de diálogos sem janela,
testes do frontend, automação, `fmt`/`clippy` e testes Rust. Foram 1.350 testes
de frontend e 120 testes de automação aprovados; três testes de automação
ficaram ignorados conforme a configuração da suíte. O relatório completo
das etapas está no campo `validation` do artefato de resultados.

A validação foi executada sobre as alterações locais; por isso o relatório
registra `sourceInputsDirty = true`. O campo `source` vincula o commit-base
aos hashes dos dois arquivos de configuração medidos. Os hashes dos três
artefatos de distribuição também estão registrados.

A suíte padrão usa o perfil de desenvolvimento e não abre janelas nativas.
Os executáveis Release foram validados pelo build e pelo experimento com o
protocolo real do Processador. A sessão gráfica da versão Release e a
instalação do NSIS não foram exercitadas nesta entrega.

As revisões de padrões e de requisitos não encontraram problemas. O arquivo
de capacidades gerado pelo Tauri permaneceu idêntico entre as variantes.

## Reprodução

Compile a referência com `npm run tauri:build -- --ci` no commit indicado e
preserve `target/release/myalbuns-desktop.exe`, o Processador preparado em
`target/sidecar-build/release/myalbuns-imaging.exe` e o instalador em
`target/release/bundle/nsis/`. Repita o mesmo comando com a configuração nova.
Mantenha o aplicativo e seu Processador correspondente juntos.

Com cinco arquivos chamados `photo-1.jpg` a `photo-5.jpg`, execute na raiz:

```powershell
node docs/research/artifacts/2026-09-17-compare-release.mjs `
  .tools/release-optimization/baseline/myalbuns-imaging.exe `
  .tools/release-optimization/optimized/myalbuns-imaging.exe `
  .tools/release-optimization/photos `
  .tools/release-optimization/comparison
```

O relatório `comparison.json` identifica o namespace exclusivo de Cache do
experimento. Os arquivos de saída ficam na pasta indicada; as prévias ficam
nesse namespace sob `%LOCALAPPDATA%/MyAlbuns2/Cache`. Esses dados são evidência
temporária, não projetos do usuário.
