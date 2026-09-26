---
status: current
document: research
date: 2026-09-23
updated: 2026-09-26
platform: windows-11-x64
---

# Latência da correção de olhos na versão local

O atraso relatado ocorre depois de selecionar os dois rostos e antes de aparecer
a foto corrigida. A detecção já terminou nesse ponto. O ensaio usa cópias locais
de um par de fotos de 4.000 × 6.000 pixels, com os mesmos pontos faciais antes e
depois. A referência de código é o commit `6745001c`.

## Resultado final

No próprio pacote desktop, a preparação passou de **13.965 ms para uma mediana
de 1.624 ms**, cerca de **88% menos tempo**, ou **8,6 vezes mais rápida** no
ensaio local. As cinco rodadas finais aquecidas, sem compilação concorrente,
levaram 1.613, 1.636, 1.610, 1.650 e 1.624 ms. Todas atenderam ao orçamento
local de 2.000 ms. O original corrigido e a prévia são idênticos byte a byte
aos resultados anteriores.

Esses números medem preparação e conferência dos arquivos na versão de
desenvolvimento. Não representam uma promessa de tempo para qualquer foto ou
máquina, nem uma medição do clique até a apresentação na janela nativa.

## Causa reproduzida

No perfil de desenvolvimento, a preparação no próprio pacote desktop levou
13.965 ms e excedeu o orçamento local de 2.000 ms. O ensaio chama o compositor
usado por `prepare_eye_correction` e inclui verificações dos arquivos antes e
depois. Não mede abertura da janela, detecção, IPC ou salvamento confirmado.

A instrumentação temporária separou os custos: leitura das duas fotos em
paralelo levou cerca de 340 ms, enquanto a redução para a prévia e a gravação
do PNG completo levaram aproximadamente 12,6 s cada, em paralelo. O ajuste
dos olhos em si levou poucos milissegundos. Não se deve somar os tempos das
etapas que executam ao mesmo tempo.

O mesmo código em um executor isolado levou 14.172 ms em desenvolvimento e
cerca de 643 ms em release. A otimização já existente de `image` não cobria
todas as operações genéricas instanciadas em `myalbuns-desktop`, nem o codec
`png`. A causa repete a diferença de perfil documentada na
[pesquisa de miniaturas](2026-09-05-latencia-das-miniaturas.md), em outro pacote.

## Experimentos controlados

| Variante no executor isolado | Resultado |
| --- | --- |
| Perfil de desenvolvimento anterior | Preparação com verificações: 14.172 ms; orçamento excedido. |
| Otimização do pacote que chama as operações de imagem | Redução para prévia caiu de 12.845 para 364 ms, mas o PNG completo ainda levou 11.758 ms. |
| Otimização do chamador e do codec PNG | Preparação com verificações: 1.709 ms; orçamento atendido. |

As comparações usam Rust/Cargo 1.98.0, `image 0.25.10` e `png 0.18.1`. A
[documentação oficial de perfis do Cargo](https://doc.rust-lang.org/cargo/reference/profiles.html#overrides-and-generics)
explica que operações genéricas podem ser compiladas no pacote que as usa.
Os ajustes pertencem ao manifesto da raiz. A consulta pelo Context7 estava sem
cota; o contrato foi conferido diretamente na documentação oficial.

## Mudança

Depois de corrigir os olhos, a imagem RGBA passa a ser envolvida uma vez em
`DynamicImage`. A redução usa `resize_exact` com o mesmo filtro Lanczos3, por
um método não genérico compilado no pacote `image`, que já era otimizado.
O caminho interno continua usando a mesma operação de redução; não muda o
algoritmo nem a representação de cor. O contrato foi conferido no código da
versão instalada `image 0.25.10`.

O perfil de desenvolvimento passa a usar `opt-level = 1` para
`myalbuns-desktop` e `opt-level = 3` para `png:0.18.1`. O codec é identificado
pela versão porque a workspace também resolve `png 0.17.16` para outros
consumidores. Uma atualização de `image` ou do codec deve revisar esse seletor
e repetir a medição.

A escolha evita o nível máximo de otimização no aplicativo inteiro. As
alternativas menores foram medidas antes da decisão:

| Alternativa | Limite da medição | Preparação com verificações |
| --- | --- | ---: |
| Código anterior, desktop 3 e PNG 3 | Pacote desktop, quatro rodadas após aquecimento | Mediana de 1.219,5 ms |
| Código anterior, chamador 1 e PNG 3 | Executor isolado | 4.039 ms |
| `DynamicImage`, desktop 0 e PNG 3 | Pacote desktop | 3.365 ms |
| `DynamicImage`, desktop 1 e PNG 3 — escolhida | Primeira execução no pacote desktop | 1.605 ms |

A alternativa escolhida atende ao orçamento local sem exigir o nível 3 no
desktop. A comparação principal de desempenho deve usar o mesmo pacote e
perfil, sem misturar números do executor isolado com os do aplicativo.

O ajuste preserva o algoritmo, a resolução completa, os metadados suportados,
a confirmação de substituição e as verificações de alteração externa. O perfil
release já era otimizado. Otimizar pacotes em desenvolvimento pode aumentar
o tempo de recompilação e dificultar a inspeção passo a passo; as verificações
de depuração e overflow continuam herdadas do perfil de desenvolvimento.

## Qualidade e exibição

O PNG completo continua com 4.000 × 6.000 pixels, e a prévia com 1.067 × 1.600.
Ambos ficaram idênticos byte a byte entre as variantes:

- SHA-256 completo: `9C75BA9F58442E277D88CAE3E2059663CAD09B073EAFB29F395A23767CD59A44`.
- SHA-256 prévia: `B78B0CCAE09C3916971FAEACA9D40AE9CC5AF89B738E7DC6249D8CC4C78E113D`.

No Edge 153, carregar a prévia de 3.217.181 bytes por HTTP local, decodificá-la
e aguardar dois quadros de apresentação levou medianas de 49,9 ms e 41,6 ms
nas duas variantes, com cinco amostras cada. Essa pequena variação não é
atribuída à mudança: os bytes são os mesmos. O ensaio isola a exibição e não
substitui uma medição ponta a ponta na janela nativa.

As fotos e saídas de diagnóstico ficam somente em
`.scratch/face-detection-debug-20260923`. Nenhuma foto original do usuário é
substituída pelo ensaio. O teste ignorado
`eye_correction::qa_tests::profiles_real_pair_in_desktop_crate` permite repetir
a preparação no pacote real; `EYE_PREPARE_BUDGET_MS=2000` habilita o orçamento
local, sem impor um tempo dependente da máquina à suíte comum.

Com as cópias locais e os pontos faciais já disponíveis, executar na raiz:

```powershell
$env:EYE_PREPARE_BUDGET_MS = '2000'
./scripts/Invoke-LocalCargo.ps1 test -p myalbuns-desktop --lib profiles_real_pair_in_desktop_crate -- --ignored --nocapture
```

## Verificação

- `cargo test -p myalbuns-desktop --quiet`: 532 testes passaram; 27 ignorados.
- O teste de desempenho ignorado foi executado explicitamente no pacote real:
  orçamento excedido antes e atendido depois.
- Hashes e dimensões da saída completa e da prévia foram conferidos novamente
  na implementação final; os arquivos de entrada permaneceram inalterados.
- Toda instrumentação temporária de estágios foi removida do código de produção.
- O antigo diagnóstico opcional com os retratos `nikki` não pôde rodar porque
  seu arquivo local `qa/faces.json` está ausente. Os testes automatizados de
  orientação EXIF, perfil de cor, restauração e preservação do original fazem
  parte da suíte que passou; esse diagnóstico adicional não é contado como
  validado.

O resumo de medições fica em
`.scratch/face-detection-debug-20260923/eye-prepare-perf-summary-20260923.json`;
o ensaio de apresentação, em `preview-browser-timings.json` no mesmo diretório.

## Prevenção

`CODING_STANDARDS.md` passa a exigir que a evidência de desempenho identifique
o perfil realmente usado, inclua o pacote que instancia operações genéricas
e diferencie tempo de processamento de latência percebida na janela. Uma
medição rápida em release não basta para declarar responsiva a versão local.

## Pedidos superados na preparação

Em 23/09/2026, a preparação passou a admitir um único trabalho ativo por Host,
coalescer pedidos do mesmo par e versão e invalidar esperas de pares superados.
O trabalho já iniciado consulta a geração antes da leitura, da composição e da
codificação do PNG. Essas consultas não interrompem uma etapa que já começou;
em especial, uma codificação em andamento ainda termina antes de liberar o
worker. O original continua protegido por hash na preparação e na aplicação.

O ensaio abaixo usa as mesmas cópias de 4.000 × 6.000 pixels e os mesmos pontos
do teste anterior, no perfil `dev` de `myalbuns-desktop`, com compilação fora
do cronômetro. A comparação de três pedidos executa o **compositor nativo em
série**: de um lado, três preparações completas; do outro, um pedido interrompido
antes da codificação, um pedido dispensado antes da leitura e o pedido atual
completo. Mede tempo de parede e CPU do processo na fronteira de preparação,
incluindo hash do original. É uma simulação controlada da supersessão, não uma
medição dos comandos Tauri concorrentes, da detecção facial nem do clique até a
prévia aparecer na janela.

| Cenário controlado | Parede | CPU do processo | PNGs completos |
| --- | ---: | ---: | ---: |
| Um par, sem supersessão | 2.309 ms | 3.015 ms | 1 |
| Três pedidos sem interrupção, em série | 7.070 ms | 9.421 ms | 3 |
| Três pedidos com checkpoints, em série | 2.575 ms | 3.875 ms | 1 |

No último caso, dois pedidos chegaram à etapa de leitura, dois à composição e
apenas o atual concluiu o PNG. As saídas completas e as prévias dos cenários
têm hashes SHA-256 idênticos, respectivamente
`9c75ba9f58442e277d88cae3e2059663cad09b073eafb29f395a23767cd59a44`
e `b78b0ccae09c3916971faeaca9d40ae9cc5af89b738e7dc6249d8cc4c78e113d`.
Os hashes das duas cópias de entrada também permaneceram iguais. A variação do
tempo de um pedido em relação à mediana anterior de 1.624 ms confirma que não
se deve juntar rodadas de condições distintas numa estimativa de ganho.

O teste ignorado reproduz essa fronteira sem alterar as fotos de entrada; cria
as saídas em uma pasta temporária dentro de `.scratch/.../render` e a remove
ao terminar:

```powershell
& ./scripts/Invoke-LocalCargo.ps1 -CargoArguments @('test','-p','myalbuns-desktop','--lib','benchmark_serial_native_render_with_supersession_checkpoints','--','--ignored','--nocapture')
```

A medição ponta a ponta na janela nativa foi feita em 26/09/2026 e está em
[Latência ponta a ponta da correção de olhos](2026-09-26-latencia-ponta-a-ponta-da-correcao-de-olhos.md).
O `preview-latency.mjs` existente mede somente a apresentação de uma URL de QA
no Edge, sem preparação no Host; seus números não substituem aquela medição.
