---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-31
updated: 2026-08-01
---

# Distribuição Windows e validação em máquina limpa

## Objetivo e estado

Este gate validou em conjunto WebView2 Evergreen, diálogo nativo de arquivo,
instalador `win-x64` e um fluxo ponta a ponta depois da instalação em uma
máquina Windows limpa.

A rodada final executou o release local e uma instalação por usuário em Windows
Sandbox descartável. As duas partes foram correlacionadas ao commit
`a76a541e90ff2a3bb26d56b1f94634632b151819` e ao mesmo SHA-256 do instalador.
O artefato canônico
[0018-windows-distribution-gate.json](artifacts/0018-windows-distribution-gate.json)
encerra o critério do ticket 01.

## Corte implementado

A Tela de Boas-vindas recebeu o port estreito `ProjectFileDialog`. O
composition root global escolhe `tauriProjectFileDialog`, que chama somente
`open()` do plugin oficial. A UI trata seleção, cancelamento e falha sem logar o
caminho e sem iniciar uma `ProjectSession`.

Não foi definido filtro de extensão: o formato definitivo do arquivo pertence
ao ticket 02, e um filtro seria apenas conveniência de UI, não validação. O
`ProjectStore` futuro continuará responsável por validar o conteúdo escolhido.

O plugin Rust é registrado somente no builder global. A capability
`global-shell` contém exatamente:

- `global-shell-logging`, limitada a `frontend_log`;
- `dialog:allow-open`.

`dialog:default`, salvar, mensagens, `fs:*` e `shell:*` não foram concedidos.
As Janelas de Projeto não registram nem recebem o plugin de diálogo.

O bundle Windows tornou explícitos:

```json
{
  "targets": ["nsis"],
  "windows": {
    "webviewInstallMode": {
      "type": "downloadBootstrapper",
      "silent": true
    },
    "nsis": {
      "installMode": "currentUser"
    }
  }
}
```

Isso mantém o runtime Evergreen atualizado pelo sistema e produz um instalador
por usuário. O bootstrapper exige rede quando o runtime não estiver presente;
o pacote não embarca a distribuição fixa nem o instalador offline de WebView2.

## Segurança e contratos

O gate de segurança foi repetido no commit `abc023d` com 223 inputs limpos. Os
sete checks passaram, e o ACL compilado confirmou que `dialog:allow-open`
resolve somente o comando `open`. O artefato atualizado é
[0016-frontend-security-gate.json](artifacts/0016-frontend-security-gate.json).

Os contratos específicos da distribuição também passaram:

| Check | Resultado |
| --- | --- |
| `GlobalShell`, adapter e fronteira Tauri | 12 testes passaram |
| Configuração Rust do bundle Windows | passou |
| Build frontend de produção | passou |
| Build Tauri `release` e NSIS | passou |
| Target informado pelo bundler | `x64` |

## Bundle local produzido

O runner `scripts/Test-WindowsDistributionGate.ps1` usa
`target/windows-distribution-gate`, exige inputs limpos e executa um único
`Invoke-LocalTauri.ps1 build` com bundle ativo. A rodada produziu:

| Artefato | Bytes | SHA-256 |
| --- | ---: | --- |
| `myalbuns-desktop.exe` | 15.584.256 | `efaf4c22b417b2b000a6c7739f3827d368140c3c57300c553c433f459454b055` |
| `myalbuns-imaging.exe` | 2.802.176 | `af639d86c3a31887bad1fe70079583b860b1d6a1369f89c9c515b169a9cecd3a` |
| Sidecar preparado para o bundle | 2.802.176 | `af639d86c3a31887bad1fe70079583b860b1d6a1369f89c9c515b169a9cecd3a` |
| `MyAlbuns_0.1.0_x64-setup.exe` | 4.217.171 | `b514ead6e0f51cd5bfa8120c784c79d94c00241b1ebd6a390764a8046bfcf946` |

O executável do aplicativo, o Processador recém-compilado e o sidecar preparado
possuem `Machine = 0x8664` e optional header `0x020b`, isto é, AMD64 PE32+.
O hash idêntico dos dois sidecars prova que o payload oferecido ao bundler veio
do build `release`. O stub do setup NSIS não é usado como prova de arquitetura;
o nome `x64` emitido pelo bundler e os payloads PE32+ são a evidência relevante.

## Runtime local observado

O release não instalado foi iniciado no papel global e chegou ao evento
`welcome_screen_ready`, à Janela `MyAlbuns` e a seis descendentes reais
`msedgewebview2.exe`. Todos usavam o runtime de sistema em:

```text
C:\Program Files (x86)\Microsoft\EdgeWebView\Application\150.0.4078.105\
```

O arquivo observado informou `ProductVersion` e `FileVersion`
`150.0.4078.105`. O runner endurecido exige caminho existente fora do
workspace e versão válida; não aceita apenas um processo com o nome esperado.

Essa observação prova o release local sobre o WebView2 de sistema. A rodada em
Windows Sandbox, descrita abaixo, cobre separadamente o aplicativo instalado.

## Diálogo nativo local

O runner emite um checkpoint e espera duas evidências independentes:

1. um recibo de Computer Use identificando a Janela nativa `Abrir Projeto`;
2. o evento estruturado `project_file_selection_cancelled` depois de cancelar.

Um retorno `null` isolado do adapter não é aceito como prova de que a Janela
nativa apareceu. O Computer Use observou a Janela `Abrir Projeto`, gravou o
recibo externo antes de enviar `Escape`, e o log estruturado confirmou
`project_file_selection_cancelled`. O runner correlacionou proprietário,
diálogo, ação solicitada e evento de cancelamento.

## E2E em ambiente descartável

O setup produzido foi copiado para um Windows Sandbox sem instalação prévia do
MyAlbuns. A execução limpa confirmou:

- Windows 11 Enterprise `10.0.26100`, com `disposable: true` e
  `preexistingMyAlbuns: false`;
- execução e conclusão do instalador NSIS por usuário;
- aplicativo instalado em
  `C:\Users\WDAGUtilityAccount\AppData\Local\MyAlbuns` e iniciado pelo binário
  instalado;
- seis processos do WebView2 Evergreen `151.0.4129.59`;
- diálogo nativo `Abrir Projeto` observado por Computer Use, recibo gravado
  antes do cancelamento e evento estruturado correspondente.

A evidência limpa importada possui SHA-256
`3de964084dc93ed65145152e0e021fba0b03c8bdc188d11c8025542992fed24b` e
repete o commit e o SHA-256 do setup local. O runner rejeita qualquer divergência
nesses valores.

No host usado para o ensaio, a configuração temporária do Sandbox com
`<VGpu>Disable</VGpu>` reproduziu a perda da sessão visual, enquanto
`<VGpu>Enable</VGpu>` manteve a conexão. Essa é uma observação específica do
ambiente de teste; o arquivo `.wsb` permanece descartável sob `target/` e não é
contrato do produto.

## Conclusão

Todos os 13 checks do artefato `0018` passaram. Os marcadores
`localProbePassed`, `cleanMachineE2ePassed`, `ticketCriterionSatisfied` e
`criterionClosed` são `true`. O gate de distribuição Windows do ticket 01 está
encerrado sem executar o instalador no host de desenvolvimento.

O fechamento não decide a topologia A/B nem encerra o spike completo. A coleta
comparativa consolidada e a recomendação final continuam nos dois critérios
seguintes do ticket.

## Repetição

O runner imprime `COMPUTER_USE_READY=<checkpoint>` quando a Janela local está
pronta. O driver deve observar `Abrir Projeto`, gravar o recibo indicado pelo
checkpoint e só então cancelar o diálogo. O limite configurável foi ampliado
para acomodar a interação local e a rodada externa sem afrouxar as validações.

Quando o E2E for executado em uma máquina descartável, a repetição local usa:

```powershell
./scripts/Test-WindowsDistributionGate.ps1 `
  -CleanMachineEvidencePath <caminho-do-recibo.json> `
  -InteractionTimeoutSeconds 1200
```

O recibo usa `schemaVersion: 1`, suite
`myalbuns_windows_clean_machine_e2e` e os seguintes campos normativos:

```json
{
  "schemaVersion": 1,
  "suite": "myalbuns_windows_clean_machine_e2e",
  "gitCommit": "<commit de 40 caracteres>",
  "installerSha256": "<sha256 do setup>",
  "collectedAtUtc": "<timestamp ISO 8601>",
  "environment": {
    "provider": "<VM ou Sandbox>",
    "disposable": true,
    "preexistingMyAlbuns": false
  },
  "results": {
    "installerExecuted": true,
    "installationPassed": true,
    "installedBinaryExercised": true,
    "appLaunched": true,
    "webView2": {
      "distribution": "Evergreen",
      "executablePath": "<caminho observado>",
      "productVersion": "<versão em quatro partes>"
    },
    "nativeDialog": {
      "observed": true,
      "kind": "native_file_open",
      "outcome": "cancelled"
    },
    "passed": true
  }
}
```
