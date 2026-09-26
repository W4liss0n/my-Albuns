---
status: current
document: research
date: 2026-09-26
platform: windows-11-x64
---

# Latência ponta a ponta da correção de olhos

Esta medição cobre o intervalo que o usuário percebe: do clique no rosto que
completa o par até a foto corrigida aparecer pintada na janela do visualizador.
Completa a [medição de 23/09/2026](2026-09-23-latencia-da-correcao-de-olhos.md),
que cronometrava apenas o compositor nativo, e responde ao último critério da
issue #124.

## Resultado

Na janela nativa, no perfil `dev`, a **mediana foi de 2.041 ms** em seis rodadas
aquecidas, entre 1.978 e 2.144 ms. A rodada de aquecimento levou 1.925 ms.

| Trecho | Mediana |
| --- | ---: |
| Clique até o pedido `prepare_eye_correction` sair da janela do Projeto | 12 ms |
| Comando `prepare_eye_correction` no Host | 1.953 ms |
| Resposta até a prévia pintada: atualização, carregamento, decodificação e dois quadros | 80 ms |
| **Total percebido** | **2.041 ms** |

A preparação no Host responde por 96% do tempo. A interface soma cerca de
90 ms: a prévia PNG de 3,2 MB carrega em cerca de 35 ms e é decodificada e
pintada em mais 39 ms. Cada clique gerou exatamente um pedido de preparação.
As duas cópias de entrada mantiveram os hashes SHA-256 depois das rodadas.

Esses números não são uma promessa de tempo para outras fotos ou máquinas.

## Método

- Aplicativo `myalbuns-desktop` compilado com `tauri build --debug --no-bundle`
  a partir do commit `2c5d54f7`, com a interface embutida. É o mesmo perfil
  `dev` do repositório, com os níveis de otimização por pacote do manifesto da
  raiz. O código da correção nesse commit é o mesmo de `main`.
- Projeto isolado criado com o Core, com cópias de `failing.jpg` e
  `reference.jpg` (6.000 × 4.000, orientação EXIF 8, 24 MP) e dados do
  aplicativo em `MYALBUNS_PROCESS_GATE_DATA_ROOT`. Nenhum dado real do usuário
  foi usado.
- Os cliques foram eventos reais de entrada enviados pelo protocolo de
  depuração do WebView2: abrir a foto com Espaço, **Abrir olhos**, **Usar esta
  foto**, clicar no rosto de destino e, cronometrado, no rosto de referência.
  Cada rodada seguinte usou **Trocar referência**, que cancela a prévia anterior.
- O início é o `pointerdown` no rosto; o fim, a nova imagem da prévia
  decodificada e seguida de dois quadros de animação, ambos no relógio do
  visualizador. Os tempos de IPC vêm da Resource Timing da janela do Projeto,
  já que o IPC do Tauri trafega como requisições a `ipc.localhost`.
- Para abrir sua porta de depuração, o visualizador usou um perfil WebView2
  separado do perfil da janela do Projeto. Isso pode alterar a fase final em
  poucos milissegundos, mas não o comando no Host.

## Onde está o tempo da preparação

As operações de `render_with_checkpoint` foram repetidas isoladamente, com as
mesmas fotos, versões (`image 0.25.10`, `png 0.18.1`, `zune-jpeg 0.5.15`) e
níveis de otimização do perfil `dev`: chamador em `opt-level = 1`, codecs em
`opt-level = 3`. O perfil `release` usa as opções do manifesto da raiz. Os
valores são medianas de cinco repetições após aquecimento.

| Etapa | `dev` | `release` |
| --- | ---: | ---: |
| Hash SHA-256 do original | 7 ms | 8 ms |
| Leitura das duas fotos em paralelo | 313 ms | 300–450 ms |
| ↳ decodificação JPEG de uma foto | 143 ms | 130 ms |
| ↳ rotação pela orientação EXIF | 125 ms | 135 ms |
| ↳ conversão para RGBA | 40 ms | 37 ms |
| Redução Lanczos3 para 1.067 × 1.600 | 303 ms | 337 ms |
| Codificação da prévia PNG | 88 ms | 19 ms |
| **Gravação do PNG completo, 35 MB** | **1.283 ms** | **647 ms** |
| Etapa final atual: prévia em paralelo com o PNG completo | 1.263 ms | 674 ms |
| Etapa final só com a prévia | 382 ms | 369 ms |

O PNG completo só é usado ao salvar a correção, mas a prévia espera sua
gravação. No perfil `dev`, isso custa cerca de 880 ms por clique.

A soma das etapas isoladas no perfil `dev` fica perto de 1,6 s. Os cerca de
350 ms restantes do comando no Host não foram atribuídos; exigem instrumentação
temporária do Host.

Alternativas medidas e descartadas:

- A compressão `Fast` do encoder PNG produziu os mesmos bytes e o mesmo tempo
  da padrão.
- A prévia em JPEG com qualidade 92 levou 57 ms em `release`, mais que o PNG,
  e perde fidelidade na comparação antes/depois.
- `fast_image_resize` reduziu a prévia em 121 ms em `release`, mas levou
  1.046 ms quando instanciado num pacote em `opt-level = 1`. Só compensa se a
  redução rodar num pacote otimizado.

## Oportunidades

Estimativas a partir das etapas isoladas; o ganho real exige nova medição
ponta a ponta:

1. **Não gravar o PNG completo na preparação.** Cerca de 0,9 s a menos no
   perfil `dev` e 0,3 s em `release`. Também elimina 35 MB temporários por
   prévia. Implementada na #131; ver o resultado abaixo.
2. **Redução mais rápida da prévia.** Cerca de 200 ms, com `fast_image_resize`
   num pacote otimizado ou redução em duas etapas. Altera apenas os bytes da
   prévia.
3. **Evitar a rotação completa e trabalhar em RGB.** Cerca de 150 ms em fotos
   com orientação EXIF, mapeando coordenadas em vez de girar os pixels.

## Arquivos órfãos

Quando o aplicativo foi encerrado à força com uma prévia pendente, o PNG de
35 MB permaneceu em `.myalbuns-corrections`, ao lado do Projeto, nos três
encerramentos forçados do ensaio. Nenhum código remove esses arquivos depois.
Com **Trocar referência**, os seis PNGs intermediários foram removidos
normalmente. Desde a #131 a preparação não grava arquivos, e o problema deixa
de existir.

## Resultado da #131: refazer a imagem ao salvar

A preparação passou a devolver somente a prévia. Ao confirmar **Substituir
original**, o Host confere os hashes da foto a corrigir e da referência,
refaz a correção a partir das duas e a codifica no formato do original.

A comparação usou as mesmas cópias, o mesmo perfil `dev` e o mesmo roteiro,
agora estendido até **Salvar correção** → **Substituir original**. Versão
anterior (`2c5d54f7`) e nova foram executadas alternadamente, três vezes cada,
com projeto e dados novos em cada rodada. Outra sessão compilava na mesma
máquina durante o ensaio, o que afeta as duas versões; por isso a alternância.

| Trecho, mediana de 3 rodadas | Antes | Depois |
| --- | ---: | ---: |
| Clique → prévia pintada | 2.108 ms | 1.046 ms |
| ↳ comando `prepare_eye_correction` | 1.985 ms | 911 ms |
| **Substituir original** → correção encerrada | 2.597 ms | 1.997 ms |
| ↳ comando `apply_eye_correction` | 2.524 ms | 1.937 ms |

O Salvar também ficou mais rápido: refazer a composição custa menos que ler
de volta o PNG de 35 MB. As seis fotos substituídas têm o mesmo SHA-256,
`7857216ee0eb76731480f968a90db7bd621b4e89cb3f70f19e5c50e55ba7a686`, e a prévia
continua idêntica byte a byte à de 23/09/2026 (`b78b0cca…`). A versão nova não
criou `.myalbuns-corrections` em nenhuma rodada, nem ao ser encerrada à força
com uma prévia pendente; nesse caso, as duas fotos mantiveram seus hashes.
Com prévias seguidas no mesmo aplicativo, o clique → prévia ficou entre 903 e
976 ms.

## Reprodução

O roteiro, o gerador do projeto e o ensaio por etapas ficam somente em
`.scratch/eye-latency-20260926/tools`; os resultados, em `native-dev.json`,
`stage-bench-dev.txt` e `stage-bench-release.txt` na mesma pasta. A comparação
da #131 usa `tools/apply-run.sh` e fica em `apply-cmp-*`. As fotos
de entrada vêm de `.scratch/face-detection-debug-20260923/inputs`.

A execução visível abre janelas do aplicativo e exige autorização explícita,
conforme `docs/agents/native-ui-gates.md`. Esta rodada foi autorizada pelo
autor em 26/09/2026.
