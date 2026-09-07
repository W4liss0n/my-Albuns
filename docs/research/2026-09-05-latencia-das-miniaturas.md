---
status: current
document: research
date: 2026-09-05
updated: 2026-09-05
platform: windows-11-x64
---

# Latência das miniaturas na versão local

Depois da correção de compatibilidade JPEG, o Painel ainda demorava para mostrar
a miniatura após a importação. O log de uso registrou aproximadamente cinco
segundos entre o início e o fim da geração de uma única prévia.

## Diagnóstico e mudança

O aplicativo local executava o Processador e os codecs com a otimização padrão
de desenvolvimento desabilitada. Uma medição temporária no caminho real do
cache mostrou que decodificar e reduzir os pixels consumia a maior parte do
tempo; a leitura e a verificação integral dos arquivos eram menores.

O perfil `dev` agora usa `opt-level = 3` para `myalbuns-imaging`, `image`,
`zune-jpeg` e `sha2`. Incluir o próprio Processador também otimiza operações
genéricas de imagem instanciadas nele. As configurações ficam no `Cargo.toml`
da raiz, conforme a [documentação de perfis do Cargo](https://doc.rust-lang.org/cargo/reference/profiles.html#overrides),
consultada para a toolchain Rust/Cargo 1.98.0 e as dependências resolvidas
`image 0.25.10`, `zune-jpeg 0.5.15` e `sha2 0.10.9`.

A mudança não altera o algoritmo, a política de cache, a resolução, a qualidade
JPEG, a interpretação de cor nem as verificações dos Originais. As opções de
depuração e overflow continuam herdadas de `dev`; `release` já tinha otimização
habilitada. A primeira recompilação dessas dependências custa mais tempo; as
compilações seguintes reutilizam seus artefatos.

## Comparação no Processador real

As cinco Fotos anteriores, de 3603 × 2776, foram processadas em namespaces de
cache vazios, na mesma máquina. O intervalo vem dos eventos
`cache_request_started` e `cache_request_completed`, por pedido.

| Foto | Antes | Otimizada, com medição por etapa | Final, sem instrumentação |
|---|---:|---:|---:|
| 1 | 4,401 s | 0,330 s | 0,225 s |
| 2 | 4,927 s | 0,558 s | 0,264 s |
| 3 | 6,552 s | 0,308 s | 0,231 s |
| 4 | 12,085 s | 0,415 s | 0,232 s |
| 5 | 11,145 s | 0,339 s | 0,240 s |

O orçamento local de dois segundos por prévia falhava antes e passou para as
cinco Fotos nas duas execuções otimizadas. Tempos variam com a carga da máquina;
esta é uma medição da geração no Processador, não da apresentação final na
janela nativa. O ensaio completo inicial, incluindo o programa de diagnóstico,
abertura dos resultados e verificações, levou 44,42 s; esse total não é usado
como tempo de geração de uma miniatura.

As cinco prévias finais são idênticas byte a byte às anteriores e mantêm
1600 × 1233. Todos os Originais permaneceram inalterados. Os dados pessoais e
os resultados detalhados ficam somente em `.tools/import-preview-diagnosis`.
A instrumentação temporária foi removida.

## Verificação

A preparação do Processador local é `npm run sidecar:prepare`. As verificações
afetadas são `npm run quality:rust` e `npm run test:rust`, com os testes que abrem
janelas desabilitados. Elas exercitam os mesmos contratos e fixtures anteriores
com o novo perfil de compilação. O relatório local em
`.tools/validation/preview-latency.json` registra o commit e os resultados.

Não foi necessário alterar componentes visuais ou o protocolo de prévias para
remover o gargalo reproduzido com uma única Foto. O cronômetro por pedido no
Processador, além das medições em `release` já existentes, teria exposto a
diferença de desempenho no executável realmente usado durante o desenvolvimento.
