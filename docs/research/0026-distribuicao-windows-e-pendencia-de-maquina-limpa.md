---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-31
updated: 2026-07-31
---

# Distribuição Windows e pendência de máquina limpa

## Objetivo e estado

Este gate precisa validar em conjunto WebView2 Evergreen, diálogo nativo de
arquivo, instalador `win-x64` e um fluxo ponta a ponta depois da instalação em
uma máquina Windows limpa.

O corte local avançou, mas o critério permanece aberto. O bundle NSIS, seus
payloads x64, a configuração Evergreen, a ACL do diálogo e o runtime WebView2
foram exercitados. O recibo visual do diálogo foi bloqueado pela área de
trabalho travada, e este host não possui ambiente Windows descartável no qual
instalar o pacote.

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
| `myalbuns-desktop.exe` | 15.584.256 | `c3820c947af4f2871861345a9c5be317cfd2272586f9df1a7e7de38495625a5a` |
| `myalbuns-imaging.exe` | 2.802.176 | `af639d86c3a31887bad1fe70079583b860b1d6a1369f89c9c515b169a9cecd3a` |
| Sidecar preparado para o bundle | 2.802.176 | `af639d86c3a31887bad1fe70079583b860b1d6a1369f89c9c515b169a9cecd3a` |
| `MyAlbuns_0.1.0_x64-setup.exe` | 4.221.187 | `0c78ef701cfb11cec15d2d47ed9c37bd68e09f2ac167b49c7e5063b783c2c4a7` |

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

Essa rodada prova o release local sobre WebView2 de sistema. Ela não prova que
o bootstrapper consegue instalar ou atualizar o runtime em uma máquina sem
WebView2.

## Diálogo nativo: evidência ainda pendente

O runner emite um checkpoint e espera duas evidências independentes:

1. um recibo de Computer Use identificando a Janela nativa `Abrir Projeto`;
2. o evento estruturado `project_file_selection_cancelled` depois de cancelar.

Um retorno `null` isolado do adapter não é aceito como prova de que a Janela
nativa apareceu. Nesta execução, o Computer Use encontrou a tela de login do
Windows e, por segurança, não tentou desbloqueá-la nem fornecer credenciais. O
runner expirou após cinco minutos, falhou explicitamente, encerrou toda a árvore
e não escreveu o artefato canônico `0018`. Assim, o diálogo está coberto por
contratos e ACL, mas sua observação visual real continua pendente.

## Ausência de ambiente descartável

A inspeção somente leitura encontrou:

- `Containers-DisposableClientVM` e Windows Sandbox desabilitados, sem
  `WindowsSandbox.exe`;
- Hyper-V, Virtual Machine Platform, Hypervisor Platform, Containers e WSL
  desabilitados;
- nenhuma VM ou ferramenta utilizável de Hyper-V, VMware, VirtualBox, Docker
  ou Sandboxie.

O hipervisor observado pertence ao VBS/HVCI de segurança e não oferece uma VM
para o gate. Criar outro usuário, trocar apenas `%LOCALAPPDATA%` ou instalar no
host atual não equivale a máquina limpa, pois continuaria compartilhando
Windows, HKLM e o runtime WebView2.

Nenhum instalador foi executado no host. Para fechar o critério será necessário
habilitar Windows Sandbox com privilégios administrativos e possível reinício,
ou fornecer uma VM Windows restaurável a partir de snapshot.

## Conclusão

O corte local é tecnicamente viável e o instalador `win-x64` existe, mas o
critério do ticket 01 não está encerrado. Faltam duas confirmações externas:

1. desbloquear a área de trabalho e repetir o recibo visual do diálogo;
2. instalar e executar o setup em Windows descartável realmente limpo.

Somente depois delas o runner poderá publicar
`artifacts/0018-windows-distribution-gate.json` com
`ticketCriterionSatisfied: true`.

## Repetição

```powershell
npm run spike:windows-distribution
```

O runner imprime `COMPUTER_USE_READY=<checkpoint>` quando a Janela está pronta.
O driver deve observar `Abrir Projeto`, gravar o recibo indicado pelo
checkpoint e cancelar o diálogo. Sem esse recibo ou sem ambiente limpo, o gate
permanece aberto.
