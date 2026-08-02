param(
    [switch] $SkipBuild,
    [ValidateRange(10, 120)]
    [int] $WindowTimeoutSeconds = 45,
    [ValidateRange(30, 1800)]
    [int] $CacheTimeoutSeconds = 900,
    [ValidateRange(30, 1800)]
    [int] $PerformanceTimeoutSeconds = 300,
    [ValidateSet('AB', 'BA')]
    [string] $ExecutionOrder = 'AB',
    [string] $ImagingRecoveryPath,
    [string] $OutputPath
)

$ErrorActionPreference = 'Stop'
$ExecutionOrder = $ExecutionOrder.ToUpperInvariant()
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain
. (Join-Path $PSScriptRoot 'Evidence-BuildInputs.ps1')
. (Join-Path $PSScriptRoot 'Topology-FailureProbe.ps1')

$targetDirectory = Join-Path $script:WorkspaceRoot '.scratch\topology-spike-target'
$executablePath = Join-Path $targetDirectory 'release\myalbuns-desktop.exe'
$imagingExecutablePath = Join-Path $targetDirectory 'release\myalbuns-imaging.exe'
$executableRelativePath = '.scratch/topology-spike-target/release/myalbuns-desktop.exe'
$imagingExecutableRelativePath = '.scratch/topology-spike-target/release/myalbuns-imaging.exe'
$buildManifestPath = Join-Path $targetDirectory 'topology-build-manifest.json'
$topologyEnvironment = 'MYALBUNS_TOPOLOGY_SPIKE'
$projectSlotEnvironment = 'MYALBUNS_TOPOLOGY_PROJECT'
$topologyRunIdEnvironment = 'MYALBUNS_TOPOLOGY_RUN_ID'
$processRoleEnvironment = 'MYALBUNS_PROCESS_ROLE'
$globalEndpointEnvironment = 'MYALBUNS_GLOBAL_SPIKE_ENDPOINT'
$corpusManifestEnvironment = 'MYALBUNS_TOPOLOGY_CORPUS_MANIFEST'
$probeGateEnvironment = 'MYALBUNS_TOPOLOGY_PROBE_GATE'
$exportGateEnvironment = 'MYALBUNS_TOPOLOGY_EXPORT_GATE'
$faultGateEnvironment = 'MYALBUNS_TOPOLOGY_FAULT_GATE'
$faultOutputRootEnvironment = 'MYALBUNS_TOPOLOGY_FAULT_OUTPUT_ROOT'
$projectASourceEnvironment = 'MYALBUNS_TOPOLOGY_PROJECT_A_SOURCE'
$projectBSourceEnvironment = 'MYALBUNS_TOPOLOGY_PROJECT_B_SOURCE'
$graphicsContextLossMechanism = 'webgl_lose_context'
$graphicsTestedMediaId = 'decorative-overlay'
$graphicsTestedTextureWidthPx = 1600
$graphicsTestedTextureHeightPx = 1200
$rgba8BytesPerPixel = 4
$corpusManifestPath = Join-Path $script:WorkspaceRoot '.scratch\topology-corpus\manifest.json'
$probeGateDirectory = Join-Path $script:WorkspaceRoot '.scratch\topology-probe-gates'
$desktopLogDirectory = Join-Path $env:LOCALAPPDATA 'MyAlbuns2\Logs'
$startedProcessIds = [System.Collections.Generic.List[int]]::new()
$probeGatePaths = [System.Collections.Generic.List[string]]::new()

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path `
        $script:WorkspaceRoot `
        'docs\research\artifacts\0007-process-failure-gate.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)

if ([string]::IsNullOrWhiteSpace($ImagingRecoveryPath)) {
    $ImagingRecoveryPath = Join-Path `
        $script:WorkspaceRoot `
        'docs\research\artifacts\0004-imaging-recovery.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($ImagingRecoveryPath)) {
    $ImagingRecoveryPath = Join-Path `
        $script:WorkspaceRoot `
        $ImagingRecoveryPath
}
$ImagingRecoveryPath = [System.IO.Path]::GetFullPath($ImagingRecoveryPath)

$reportText = @'
{
  "notMeasured": [
    "aloca\u00e7\u00e3o sint\u00e9tica em MAX_TEXTURE_SIZE ao quadrado ou indu\u00e7\u00e3o de OOM",
    "pico temporal de mem\u00f3ria gr\u00e1fica e or\u00e7amento global do driver",
    "checkpoint autom\u00e1tico de altera\u00e7\u00f5es ainda n\u00e3o salvas e restaura\u00e7\u00e3o de gesto em andamento"
  ],
  "notes": [
    "O Cache foi reconstru\u00eddo a frio com uma representa\u00e7\u00e3o de at\u00e9 1600 px por m\u00eddia: JPEG para Fotos opacas e PNG para o Decorativo transparente.",
    "Cada Canvas consultou os limites WebGL2 sem tentar alocar MAX_TEXTURE_SIZE ao quadrado nem provocar OOM.",
    "O gate gr\u00e1fico usou o Decorativo PNG real de 1600 x 1200 px, for\u00e7ou perda e restaura\u00e7\u00e3o pelo WEBGL_lose_context e confirmou novamente as texturas reais de Foto e Decorativo.",
    "Pan e Zoom foram medidos separadamente sobre uma textura real de Foto depois de 24 frames de aquecimento.",
    "A navega\u00e7\u00e3o percorreu 10 vezes a primeira, a 50\u00aa, a 100\u00aa e de volta \u00e0 primeira L\u00e2mina, aguardando a textura real do destino e o frame renderizado pelo PixiJS.",
    "Pixels residentes s\u00e3o contados pelas dimens\u00f5es das texturas materializadas; o volume RGBA8 \u00e9 uma estimativa mec\u00e2nica de quatro bytes por pixel, n\u00e3o uma leitura da mem\u00f3ria do driver.",
    "As duas Janelas iniciaram o probe pelo mesmo arquivo-gate, somente depois da conclus\u00e3o do Cache frio.",
    "A Exporta\u00e7\u00e3o mediu a primeira L\u00e2mina do \u00c1lbum principal a 300 DPI, lendo e verificando as Fotos JPEG e o Decorativo PNG originais.",
    "Cada uso valida o tamanho e o SHA-256 da m\u00eddia; o corpus completo foi recalculado depois das duas alternativas.",
    "A mem\u00f3ria inclui o host e todos os processos descendentes observados.",
    "O snapshot de mem\u00f3ria gr\u00e1fica do Windows foi capturado depois de todos os probes de Canvas e antes de liberar a Exporta\u00e7\u00e3o; ele n\u00e3o representa um pico.",
    "A Exporta\u00e7\u00e3o foi liberada por um segundo gate somente depois dos dois probes de Canvas e desse snapshot gr\u00e1fico.",
    "Os dois hosts independentes foram iniciados antes da espera pelas Janelas, usando o mesmo marco inicial da alternativa multiwindow.",
    "A queda s\u00f3 \u00e9 for\u00e7ada depois de validar o caminho do execut\u00e1vel do PID alvo.",
    "O gate de falhas aplica uma inten\u00e7\u00e3o real, persiste atomicamente, rel\u00ea pelo n\u00facleo e s\u00f3 ent\u00e3o confirma a revis\u00e3o como salva.",
    "A recupera\u00e7\u00e3o do host reabre apenas a \u00faltima revis\u00e3o explicitamente salva; recupera\u00e7\u00e3o de altera\u00e7\u00f5es n\u00e3o salvas permanece fora deste gate.",
    "A IPC \u00e9 descrita por limites, rela\u00e7\u00f5es e contagens m\u00ednimas observ\u00e1veis; nenhum escore sint\u00e9tico de complexidade foi inventado.",
    "A evid\u00eancia do Processador de Imagens \u00e9 validada e referenciada pelo artefato informado ao runner, sem duplicar seu mecanismo de queda."
  ],
  "summary": {
    "title": "Falhas controladas e isolamento das topologias de processo",
    "collected": "Coletado em UTC",
    "executionOrder": "Ordem de execu\u00e7\u00e3o",
    "raw": "JSON bruto",
    "measure": "Medida",
    "independent": "A \u2014 hosts independentes",
    "multiwindow": "B \u2014 host multiwindow",
    "hosts": "Hosts do Projeto",
    "windows": "Janelas do Projeto",
    "processes": "Processos na \u00e1rvore",
    "workingSet": "Working set agregado",
    "privateMemory": "Mem\u00f3ria privada agregada",
    "gpuPostProbeDedicated": "Snapshot Windows p\u00f3s-probe: mem\u00f3ria gr\u00e1fica dedicada",
    "gpuPostProbeShared": "Snapshot Windows p\u00f3s-probe: mem\u00f3ria gr\u00e1fica compartilhada",
    "gpuPostProbeTotal": "Snapshot Windows p\u00f3s-probe: uso dedicado + compartilhado dos processos",
    "twoWindows": "Duas Janelas identificadas",
    "afterCrash": "Janelas depois da queda for\u00e7ada",
    "cacheReadyTime": "Duas Janelas com Cache pronto",
    "canvasReadyTime": "Dois Canvas com texturas prontos",
    "cacheWallTime": "Dura\u00e7\u00e3o de parede do Cache frio",
    "cachePhotos": "Fotos processadas pelo Cache",
    "cacheDecoratives": "Decorativos PNG processados pelo Cache",
    "cacheThroughput": "Vaz\u00e3o agregada dos originais",
    "cacheSize": "Representa\u00e7\u00f5es reduzidas",
    "panP95": "Pan: pior p95 entre Projetos",
    "panOver33": "Pan: frames acima de 33 ms",
    "zoomP95": "Zoom: pior p95 entre Projetos",
    "zoomOver33": "Zoom: frames acima de 33 ms",
    "webglVersion": "Canvas: vers\u00e3o WebGL confirmada",
    "maxTextureSize": "Canvas: GL_MAX_TEXTURE_SIZE consultado (m\u00ednimo)",
    "maxRenderbufferSize": "Canvas: GL_MAX_RENDERBUFFER_SIZE consultado (m\u00ednimo)",
    "maxTextureImageUnits": "Canvas: GL_MAX_TEXTURE_IMAGE_UNITS consultado (m\u00ednimo)",
    "testedTexture": "Canvas: textura real exercitada",
    "contextRecovery": "Canvas: contextos perdidos e restaurados",
    "recoveryDuration": "Canvas: maior dura\u00e7\u00e3o observada da recupera\u00e7\u00e3o",
    "restoredFrameLatency": "Canvas: maior lat\u00eancia observada do frame restaurado",
    "navigationP95": "Navega\u00e7\u00e3o: pior p95 entre Projetos",
    "navigationOver33": "Navega\u00e7\u00e3o: respostas acima de 33 ms",
    "navigationResidentSheets": "Navega\u00e7\u00e3o: pico de L\u00e2minas residentes",
    "navigationResidentTextures": "Navega\u00e7\u00e3o: pico de texturas residentes",
    "navigationResidentTexturePixels": "Navega\u00e7\u00e3o: soma dos picos de pixels residentes por Projeto",
    "navigationResidentTextureRgba8": "Navega\u00e7\u00e3o: estimativa RGBA8 da soma dos picos por Projeto",
    "exportDuration": "Exporta\u00e7\u00e3o: dura\u00e7\u00e3o",
    "exportDimensions": "Exporta\u00e7\u00e3o: dimens\u00f5es a 300 DPI",
    "exportSources": "Exporta\u00e7\u00e3o: volume dos originais",
    "exportOutput": "Exporta\u00e7\u00e3o: tamanho do PNG",
    "corpus": "Corpus real",
    "corpusAlbums": "\u00c1lbuns",
    "corpusPhotos": "Fotos JPEG",
    "corpusDecoratives": "Decorativos PNG",
    "corpusSourceVolume": "Volume dos originais",
    "corpusDigest": "Digest do corpus",
    "corpusIntegrity": "Integridade antes/depois",
    "corpusIntegrityValue": "confirmada por SHA-256",
    "previewPolicy": "Pol\u00edtica da representa\u00e7\u00e3o",
    "previewPolicyValue": "uma pr\u00e9via por m\u00eddia (JPEG opaco ou PNG transparente), com aresta m\u00e1xima de {0} px",
    "notApplicable": "n\u00e3o se aplica",
    "otherPreserved": "outra Janela preservada",
    "build": "Build medida",
    "commit": "Commit do c\u00f3digo",
    "builtAt": "Build conclu\u00edda em UTC",
    "profile": "Perfil",
    "workingTreeDirty": "\u00c1rvore de trabalho tinha mudan\u00e7as alheias",
    "buildInputsDirty": "Entradas da build tinham mudan\u00e7as",
    "buildInputCount": "Arquivos de entrada",
    "buildInputDigest": "Digest das entradas",
    "executableHash": "Hash do host",
    "imagingExecutableHash": "Hash do Processador de Imagens",
    "checkoutMatches": "Checkout atual corresponde ao manifesto",
    "yes": "sim",
    "no": "n\u00e3o",
    "environment": "Ambiente registrado",
    "operatingSystem": "Sistema",
    "cpu": "Processador",
    "physicalMemory": "Mem\u00f3ria f\u00edsica",
    "notMeasured": "Campos ainda n\u00e3o medidos",
    "notes": "Observa\u00e7\u00f5es",
    "failureSection": "Falhas controladas",
    "evidence": "Evid\u00eancia",
    "globalOwnWindows": "Janelas pr\u00f3prias do processo global leve",
    "globalWorkingSet": "Working set do processo global leve",
    "windowsWhileGlobalUnavailable": "Janelas preservadas com o processo global indispon\u00edvel",
    "projectsWhileGlobalUnavailable": "Projetos editados, salvos e relidos com o global indispon\u00edvel",
    "globalExplicitRestart": "Rein\u00edcio global expl\u00edcito trocou o PID",
    "projectsAfterGlobalRestart": "Projetos editados, salvos e relidos ap\u00f3s o rein\u00edcio global",
    "windowsAfterHostFailure": "Janelas depois da queda do host",
    "savedRevisionReopened": "\u00daltima revis\u00e3o salva reaberta ap\u00f3s rein\u00edcio expl\u00edcito do host",
    "interruptedGlobalLinks": "Rela\u00e7\u00f5es host \u2192 global interrompidas pela queda global",
    "minimumHostCommands": "Comandos m\u00ednimos ao host por probe de Projeto",
    "minimumCorrelatedInteractions": "Intera\u00e7\u00f5es correlacionadas m\u00ednimas, incluindo status global",
    "probeFailureEvents": "Eventos de falha do probe",
    "ipcExplanation": "Os quatro comandos m\u00ednimos por probe s\u00e3o `topology_fault_probe_config`, `project_state`, `apply_project_intent` e `persist_topology_fault_probe`. A quinta intera\u00e7\u00e3o \u00e9 o status tipado do host para o processo global. Polls adicionais n\u00e3o s\u00e3o estimados.",
    "imagingSection": "Processador de Imagens",
    "validatedArtifact": "Artefato validado",
    "artifactSha256": "SHA-256",
    "cacheRecovered": "Cache recuperou ap\u00f3s um rein\u00edcio expl\u00edcito",
    "exportFailedSafely": "Exporta\u00e7\u00e3o falhou com seguran\u00e7a at\u00e9 o retry expl\u00edcito",
    "sameTopologyBuildCommit": "Mesmo commit da build topol\u00f3gica"
  }
}
'@ | ConvertFrom-Json

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public sealed class MyAlbunsWindowInfo
{
    public long Handle { get; set; }
    public int ProcessId { get; set; }
    public string Title { get; set; }
}

public static class MyAlbunsWindowProbe
{
    private delegate bool EnumWindowsCallback(IntPtr handle, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsCallback callback, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr handle);

    [DllImport("user32.dll")]
    private static extern int GetWindowTextLength(IntPtr handle);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowText(IntPtr handle, StringBuilder title, int maximumCount);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr handle, out uint processId);

    public static MyAlbunsWindowInfo[] VisibleWindowsFor(int[] processIds)
    {
        var expected = new HashSet<int>(processIds);
        var windows = new List<MyAlbunsWindowInfo>();
        EnumWindows(delegate(IntPtr handle, IntPtr parameter)
        {
            uint processId;
            GetWindowThreadProcessId(handle, out processId);
            if (!expected.Contains((int)processId) || !IsWindowVisible(handle))
            {
                return true;
            }

            var length = GetWindowTextLength(handle);
            if (length == 0)
            {
                return true;
            }

            var title = new StringBuilder(length + 1);
            GetWindowText(handle, title, title.Capacity);
            windows.Add(new MyAlbunsWindowInfo
            {
                Handle = handle.ToInt64(),
                ProcessId = (int)processId,
                Title = title.ToString()
            });
            return true;
        }, IntPtr.Zero);
        return windows.ToArray();
    }
}
'@

function Set-ProcessEnvironmentValue {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,
        [AllowNull()]
        [string] $Value
    )

    [System.Environment]::SetEnvironmentVariable(
        $Name,
        $Value,
        [System.EnvironmentVariableTarget]::Process
    )
}

function Reset-TopologyCache {
    if (-not (Test-Path -LiteralPath $imagingExecutablePath -PathType Leaf)) {
        throw "Imaging processor not found at $imagingExecutablePath."
    }
    $protocolVersionText = (
        & $imagingExecutablePath --protocol-version
    ).Trim()
    if (
        $LASTEXITCODE -ne 0 -or
        $protocolVersionText -notmatch '^[0-9]+$'
    ) {
        throw 'The native imaging processor did not report a valid protocol version.'
    }
    $requestId = "topology-cache-reset-$([DateTime]::UtcNow.Ticks)"
    $command = [ordered]@{
        kind = 'resetCache'
        request = [ordered]@{
            protocolVersion = [int]$protocolVersionText
            requestId = $requestId
            projectIds = @('project-spike-001', 'project-spike-002')
        }
    }
    $responseText = (
        $command |
            ConvertTo-Json -Depth 4 -Compress |
            & $imagingExecutablePath
    )
    if ($LASTEXITCODE -ne 0) {
        throw 'The native imaging processor could not reset the topology Cache.'
    }
    $responseEvent = $responseText | ConvertFrom-Json
    $response = $responseEvent.payload
    if (
        $responseEvent.kind -ne 'response' -or
        $response.kind -ne 'cacheReset' -or
        $response.requestId -ne $requestId -or
        $response.removedCount -notin @(0, 1, 2)
    ) {
        throw 'The native imaging processor returned an invalid Cache reset response.'
    }
}

function New-TopologyBuildManifest {
    if (-not (Test-Path -LiteralPath $executablePath -PathType Leaf)) {
        throw "Topology spike executable not found at $executablePath."
    }
    if (-not (Test-Path -LiteralPath $imagingExecutablePath -PathType Leaf)) {
        throw "Imaging processor not found at $imagingExecutablePath."
    }

    $inputState = Get-BuildInputState
    $workingTreeStatus = @(& git -C $script:WorkspaceRoot status --short)
    if ($LASTEXITCODE -ne 0) {
        throw 'Could not inspect the topology build working tree.'
    }
    $manifest = [ordered]@{
        manifestVersion = 1
        builtAtUtc = [DateTime]::UtcNow.ToString('o')
        gitCommit = (& git -C $script:WorkspaceRoot rev-parse HEAD).Trim()
        workingTreeDirty = $workingTreeStatus.Count -gt 0
        buildInputsDirty = $inputState.dirty
        buildInputFileCount = $inputState.fileCount
        buildInputDigestSha256 = $inputState.digestSha256
        executable = $executableRelativePath
        executableSha256 = (
            Get-FileHash -LiteralPath $executablePath -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        imagingExecutable = $imagingExecutableRelativePath
        imagingExecutableSha256 = (
            Get-FileHash -LiteralPath $imagingExecutablePath -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        profile = 'release'
    }
    $manifestJson = $manifest | ConvertTo-Json -Depth 4
    [System.IO.File]::WriteAllText(
        $buildManifestPath,
        $manifestJson + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    return $manifest
}

function Read-TopologyBuildManifest {
    if (-not (Test-Path -LiteralPath $buildManifestPath -PathType Leaf)) {
        throw (
            "Topology build manifest not found at $buildManifestPath. " +
            'Run without -SkipBuild first.'
        )
    }
    if (-not (Test-Path -LiteralPath $executablePath -PathType Leaf)) {
        throw "Topology spike executable not found at $executablePath."
    }
    if (-not (Test-Path -LiteralPath $imagingExecutablePath -PathType Leaf)) {
        throw "Imaging processor not found at $imagingExecutablePath."
    }

    $manifest = Get-Content `
        -LiteralPath $buildManifestPath `
        -Raw `
        -Encoding utf8 |
            ConvertFrom-Json
    $executableHash = (
        Get-FileHash -LiteralPath $executablePath -Algorithm SHA256
    ).Hash.ToLowerInvariant()
    if ($executableHash -ne $manifest.executableSha256) {
        throw (
            'Topology executable does not match its build manifest. ' +
            'Run without -SkipBuild.'
        )
    }
    $imagingExecutableHash = (
        Get-FileHash -LiteralPath $imagingExecutablePath -Algorithm SHA256
    ).Hash.ToLowerInvariant()
    if ($imagingExecutableHash -ne $manifest.imagingExecutableSha256) {
        throw (
            'Imaging processor does not match its build manifest. ' +
            'Run without -SkipBuild.'
        )
    }
    return $manifest
}

function Start-TopologyProcess {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [ValidateSet('a', 'b')]
        [string] $ProjectSlot,
        [Parameter(Mandatory = $true)]
        [string] $ProbeGatePath,
        [Parameter(Mandatory = $true)]
        [string] $ExportGatePath,
        [Parameter(Mandatory = $true)]
        [string] $RunId,
        [Parameter(Mandatory = $true)]
        [System.Net.IPEndPoint] $GlobalEndpoint,
        [Parameter(Mandatory = $true)]
        [string] $FaultGatePath,
        [Parameter(Mandatory = $true)]
        [string] $FaultOutputRoot,
        [string] $ProjectASource,
        [string] $ProjectBSource
    )

    $environmentNames = @(
        $processRoleEnvironment,
        $topologyEnvironment,
        $projectSlotEnvironment,
        $topologyRunIdEnvironment,
        $globalEndpointEnvironment,
        $corpusManifestEnvironment,
        $probeGateEnvironment,
        $exportGateEnvironment,
        $faultGateEnvironment,
        $faultOutputRootEnvironment,
        $projectASourceEnvironment,
        $projectBSourceEnvironment
    )
    $previousValues = @{}
    foreach ($name in $environmentNames) {
        $previousValues[$name] = [System.Environment]::GetEnvironmentVariable(
            $name,
            [System.EnvironmentVariableTarget]::Process
        )
    }
    try {
        Set-ProcessEnvironmentValue -Name $processRoleEnvironment -Value $null
        Set-ProcessEnvironmentValue -Name $topologyEnvironment -Value $Topology
        Set-ProcessEnvironmentValue -Name $topologyRunIdEnvironment -Value $RunId
        Set-ProcessEnvironmentValue `
            -Name $globalEndpointEnvironment `
            -Value $GlobalEndpoint.ToString()
        Set-ProcessEnvironmentValue `
            -Name $corpusManifestEnvironment `
            -Value $corpusManifestPath
        Set-ProcessEnvironmentValue `
            -Name $probeGateEnvironment `
            -Value $ProbeGatePath
        Set-ProcessEnvironmentValue `
            -Name $exportGateEnvironment `
            -Value $ExportGatePath
        Set-ProcessEnvironmentValue `
            -Name $faultGateEnvironment `
            -Value $FaultGatePath
        Set-ProcessEnvironmentValue `
            -Name $faultOutputRootEnvironment `
            -Value $FaultOutputRoot
        Set-ProcessEnvironmentValue `
            -Name $projectASourceEnvironment `
            -Value $ProjectASource
        Set-ProcessEnvironmentValue `
            -Name $projectBSourceEnvironment `
            -Value $ProjectBSource
        if ([string]::IsNullOrWhiteSpace($ProjectSlot)) {
            Set-ProcessEnvironmentValue -Name $projectSlotEnvironment -Value $null
        }
        else {
            Set-ProcessEnvironmentValue -Name $projectSlotEnvironment -Value $ProjectSlot
        }

        $process = Start-Process `
            -FilePath $executablePath `
            -WorkingDirectory $script:WorkspaceRoot `
            -PassThru
        $startedProcessIds.Add($process.Id)
        return $process
    }
    finally {
        foreach ($name in $environmentNames) {
            Set-ProcessEnvironmentValue `
                -Name $name `
                -Value $previousValues[$name]
        }
    }
}

function Assert-OwnedTopologyProcess {
    param([Parameter(Mandatory = $true)][int] $ProcessId)

    $process = Get-CimInstance Win32_Process -Filter "ProcessId = $ProcessId"
    if ($null -eq $process) {
        throw "Topology process $ProcessId is no longer running."
    }
    if (-not [string]::Equals(
        [System.IO.Path]::GetFullPath($process.ExecutablePath),
        [System.IO.Path]::GetFullPath($executablePath),
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
        throw "Process $ProcessId does not belong to the topology spike executable."
    }
}

function Stop-OwnedTopologyProcess {
    param([Parameter(Mandatory = $true)][int] $ProcessId)

    $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    if ($null -eq $process) {
        [void] $startedProcessIds.Remove($ProcessId)
        return
    }
    try {
        Assert-OwnedTopologyProcess -ProcessId $ProcessId
        Stop-Process -Id $ProcessId -Force
        Wait-Process -Id $ProcessId -Timeout 10 -ErrorAction SilentlyContinue
    }
    finally {
        if ($null -eq (
            Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
        )) {
            [void] $startedProcessIds.Remove($ProcessId)
        }
    }
}

function Wait-ForTopologyWindows {
    param(
        [Parameter(Mandatory = $true)]
        [int[]] $RootProcessIds,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedCount,
        [Parameter(Mandatory = $true)]
        [string] $ExpectedTitleMarker,
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Stopwatch] $Stopwatch
    )

    $deadline = [DateTime]::UtcNow.AddSeconds($WindowTimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        foreach ($processId in $RootProcessIds) {
            if ($null -eq (Get-Process -Id $processId -ErrorAction SilentlyContinue)) {
                throw "Topology host $processId exited before its windows were ready."
            }
        }

        $windows = [MyAlbunsWindowProbe]::VisibleWindowsFor($RootProcessIds)
        $unexpectedTitles = @(
            $windows | Where-Object {
                -not ($_.Title.Contains($ExpectedTitleMarker))
            }
        )
        if ($windows.Count -eq $ExpectedCount -and $unexpectedTitles.Count -eq 0) {
            return [ordered]@{
                elapsedMs = $Stopwatch.ElapsedMilliseconds
                windows = @($windows | Sort-Object ProcessId, Handle | ForEach-Object {
                    [ordered]@{
                        processId = $_.ProcessId
                        title = $_.Title
                    }
                })
            }
        }
        Start-Sleep -Milliseconds 100
    }

    $observedWindows = @(
        [MyAlbunsWindowProbe]::VisibleWindowsFor($RootProcessIds) |
            ForEach-Object { "$($_.ProcessId): $($_.Title)" }
    )
    throw (
        "Expected $ExpectedCount visible topology windows with marker " +
        "'$ExpectedTitleMarker' within $WindowTimeoutSeconds seconds. " +
        "Observed: $($observedWindows -join '; ')"
    )
}

function Get-MyAlbunsLogEventsSince {
    param(
        [Parameter(Mandatory = $true)]
        [DateTimeOffset] $Since,
        [Parameter(Mandatory = $true)]
        [ValidateSet(
            'myalbuns-desktop.*.jsonl',
            'myalbuns-global.*.jsonl',
            'myalbuns-imaging.*.jsonl'
        )]
        [string] $FileFilter
    )

    if (-not (Test-Path -LiteralPath $desktopLogDirectory -PathType Container)) {
        return @()
    }
    $events = [System.Collections.Generic.List[object]]::new()
    foreach ($logFile in Get-ChildItem `
        -LiteralPath $desktopLogDirectory `
        -File `
        -Filter $FileFilter) {
        $stream = $null
        $reader = $null
        try {
            $stream = [System.IO.File]::Open(
                $logFile.FullName,
                [System.IO.FileMode]::Open,
                [System.IO.FileAccess]::Read,
                [System.IO.FileShare]::ReadWrite
            )
            $reader = [System.IO.StreamReader]::new(
                $stream,
                [System.Text.Encoding]::UTF8
            )
            while (-not $reader.EndOfStream) {
                $line = $reader.ReadLine()
                if ([string]::IsNullOrWhiteSpace($line)) {
                    continue
                }
                try {
                    $event = $line | ConvertFrom-Json
                    if (
                        $null -ne $event.timestamp -and
                        [DateTimeOffset]::Parse($event.timestamp) -ge $Since
                    ) {
                        $events.Add($event)
                    }
                }
                catch {
                    # A writer may still be completing the last JSONL record.
                }
            }
        }
        finally {
            if ($null -ne $reader) {
                $reader.Dispose()
            }
            elseif ($null -ne $stream) {
                $stream.Dispose()
            }
        }
    }
    return @($events)
}

function Get-DesktopLogEventsSince {
    param([Parameter(Mandatory = $true)][DateTimeOffset] $Since)

    return @(
        Get-MyAlbunsLogEventsSince `
            -Since $Since `
            -FileFilter 'myalbuns-desktop.*.jsonl'
    )
}

function Get-GlobalLogEventsSince {
    param([Parameter(Mandatory = $true)][DateTimeOffset] $Since)

    return @(
        Get-MyAlbunsLogEventsSince `
            -Since $Since `
            -FileFilter 'myalbuns-global.*.jsonl'
    )
}

function Wait-ForMediaCache {
    param(
        [Parameter(Mandatory = $true)]
        [int[]] $RootProcessIds,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedProjectCount,
        [Parameter(Mandatory = $true)]
        [DateTimeOffset] $StartedAt,
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Stopwatch] $TopologyStopwatch
    )

    $operationPrefixes = @(
        $RootProcessIds | ForEach-Object { "cache-$($_)-" }
    )
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($CacheTimeoutSeconds)
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        foreach ($processId in $RootProcessIds) {
            if ($null -eq (Get-Process -Id $processId -ErrorAction SilentlyContinue)) {
                throw "Topology host $processId exited before its media Cache was ready."
            }
        }
        $cacheEvents = @(Get-DesktopLogEventsSince -Since $StartedAt)
        $failedEvents = @(
            $cacheEvents |
                Where-Object {
                    $candidate = $_
                    $candidate.event -eq 'media_cache_failed' -and
                    $null -ne $candidate.operation_id -and
                    @(
                        $operationPrefixes |
                            Where-Object {
                                $prefix = $_
                                $eventOperation = [string]$candidate.operation_id
                                $eventOperation.StartsWith($prefix)
                            }
                    ).Count -gt 0
                }
        )
        if ($failedEvents.Count -gt 0) {
            $failure = $failedEvents |
                Sort-Object timestamp |
                Select-Object -First 1
            throw (
                "Media Cache failed for project $($failure.project_id) " +
                "at stage $($failure.stage)."
            )
        }
        $completionEvents = @(
            $cacheEvents |
                Where-Object {
                    $candidate = $_
                    $candidate.event -eq 'media_cache_completed' -and
                    $null -ne $candidate.operation_id -and
                    @(
                        $operationPrefixes |
                            Where-Object {
                                $prefix = $_
                                $eventOperation = [string]$candidate.operation_id
                                $eventOperation.StartsWith($prefix)
                            }
                    ).Count -gt 0
                }
        )
        $projectEvents = @(
            $completionEvents |
                Group-Object project_id |
                ForEach-Object {
                    $_.Group |
                        Sort-Object timestamp |
                        Select-Object -First 1
                }
        )
        if ($projectEvents.Count -eq $ExpectedProjectCount) {
            if (
                @(
                    $projectEvents |
                        Where-Object {
                            [long]$_.decorative_media_count -ne 1 -or
                            [long]$_.decorative_artifact_count -ne 1 -or
                            [long]$_.decorative_png_artifact_count -ne 1
                        }
                ).Count -gt 0
            ) {
                throw 'The Cache did not publish exactly one PNG representation for the Decorative.'
            }
            $completedTimes = @(
                $projectEvents |
                    ForEach-Object { [DateTimeOffset]::Parse($_.timestamp) }
            )
            $startedTimes = @(
                $projectEvents |
                    ForEach-Object {
                        ([DateTimeOffset]::Parse($_.timestamp)).AddMilliseconds(
                            -[double]$_.elapsed_ms
                        )
                    }
            )
            $cacheWallTimeMs = [long](
                (($completedTimes | Sort-Object | Select-Object -Last 1) -
                    ($startedTimes | Sort-Object | Select-Object -First 1)
                ).TotalMilliseconds
            )
            $totalSourceBytes = [long](
                ($projectEvents | Measure-Object source_bytes -Sum).Sum
            )
            $mediaCount = [long](
                ($projectEvents | Measure-Object generated_count -Sum).Sum +
                ($projectEvents | Measure-Object reused_count -Sum).Sum
            )
            $decorativeCount = [long](
                ($projectEvents |
                    Measure-Object decorative_artifact_count -Sum).Sum
            )
            return [ordered]@{
                readyElapsedMs = $TopologyStopwatch.ElapsedMilliseconds
                cacheWallTimeMs = $cacheWallTimeMs
                projectCount = $projectEvents.Count
                mediaCount = $mediaCount
                photoCount = $mediaCount - $decorativeCount
                decorativeCount = $decorativeCount
                generatedCount = [long](
                    ($projectEvents | Measure-Object generated_count -Sum).Sum
                )
                reusedCount = [long](
                    ($projectEvents | Measure-Object reused_count -Sum).Sum
                )
                sourceBytes = $totalSourceBytes
                previewBytes = [long](
                    ($projectEvents | Measure-Object preview_bytes -Sum).Sum
                )
                sourceBytesPerSecond = if ($cacheWallTimeMs -gt 0) {
                    [long]($totalSourceBytes / ($cacheWallTimeMs / 1000.0))
                }
                else {
                    $null
                }
                projects = @(
                    $projectEvents |
                        Sort-Object project_id |
                        ForEach-Object {
                            [ordered]@{
                                projectId = $_.project_id
                                generatedCount = [long]$_.generated_count
                                reusedCount = [long]$_.reused_count
                                decorativeCount = [long]$_.decorative_artifact_count
                                decorativePngCount = [long]$_.decorative_png_artifact_count
                                sourceBytes = [long]$_.source_bytes
                                previewBytes = [long]$_.preview_bytes
                                elapsedMs = [long]$_.elapsed_ms
                            }
                        }
                )
            }
        }
        Start-Sleep -Milliseconds 250
    }
    throw (
        "Expected Cache completion for $ExpectedProjectCount projects within " +
        "$CacheTimeoutSeconds seconds."
    )
}

function Open-TopologyProbeGate {
    param([Parameter(Mandatory = $true)][string] $Path)

    $gateRoot = [System.IO.Path]::GetFullPath($probeGateDirectory) +
        [System.IO.Path]::DirectorySeparatorChar
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    if (-not $fullPath.StartsWith(
        $gateRoot,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
        throw 'The topology probe gate is outside its dedicated directory.'
    }
    if (Test-Path -LiteralPath $fullPath) {
        throw "The topology probe gate already exists: $fullPath"
    }
    New-Item -ItemType Directory -Force -Path $probeGateDirectory | Out-Null
    [System.IO.File]::WriteAllText(
        $fullPath,
        "ready$([System.Environment]::NewLine)",
        [System.Text.UTF8Encoding]::new($false)
    )
}

function Convert-CanvasTiming {
    [OutputType([pscustomobject])]
    param(
        [Parameter(Mandatory = $true)] $Event,
        [Parameter(Mandatory = $true)]
        [ValidateSet('pan', 'zoom', 'navigation')]
        [string] $Prefix
    )

    $sampleCount = "${Prefix}_sample_count"
    $duration = "${Prefix}_duration_ms"
    $firstFrame = "${Prefix}_first_frame_latency_ms"
    $mean = "${Prefix}_mean_frame_ms"
    $p50 = "${Prefix}_p50_frame_ms"
    $p95 = "${Prefix}_p95_frame_ms"
    $p99 = "${Prefix}_p99_frame_ms"
    $maximum = "${Prefix}_max_frame_ms"
    $over16 = "${Prefix}_frames_over16_ms"
    $over33 = "${Prefix}_frames_over33_ms"
    return [pscustomobject][ordered]@{
        sampleCount = [long] $Event.$sampleCount
        durationMs = [double] $Event.$duration
        firstFrameLatencyMs = [double] $Event.$firstFrame
        meanFrameMs = [double] $Event.$mean
        p50FrameMs = [double] $Event.$p50
        p95FrameMs = [double] $Event.$p95
        p99FrameMs = [double] $Event.$p99
        maxFrameMs = [double] $Event.$maximum
        framesOver16Ms = [long] $Event.$over16
        framesOver33Ms = [long] $Event.$over33
    }
}

function Assert-FiniteNonNegativeNumber {
    param(
        [Parameter(Mandatory = $true)][double] $Value,
        [Parameter(Mandatory = $true)][string] $Name
    )

    if (
        [double]::IsNaN($Value) -or
        [double]::IsInfinity($Value) -or
        $Value -lt 0
    ) {
        throw "$Name must be a finite non-negative number."
    }
}

function Assert-JsonNumber {
    param(
        [AllowNull()] $Value,
        [Parameter(Mandatory = $true)][string] $Name,
        [switch] $Integer
    )

    if ($null -eq $Value -or $Value -is [bool]) {
        throw "$Name must be an explicit JSON number."
    }
    $numericTypeCodes = @(
        [System.TypeCode]::Byte,
        [System.TypeCode]::SByte,
        [System.TypeCode]::Int16,
        [System.TypeCode]::UInt16,
        [System.TypeCode]::Int32,
        [System.TypeCode]::UInt32,
        [System.TypeCode]::Int64,
        [System.TypeCode]::UInt64,
        [System.TypeCode]::Single,
        [System.TypeCode]::Double,
        [System.TypeCode]::Decimal
    )
    $typeCode = [System.Type]::GetTypeCode($Value.GetType())
    if ($typeCode -notin $numericTypeCodes) {
        throw "$Name must be an explicit JSON number."
    }
    $number = [double] $Value
    if ([double]::IsNaN($number) -or [double]::IsInfinity($number)) {
        throw "$Name must be finite."
    }
    if ($Integer -and $number -ne [math]::Truncate($number)) {
        throw "$Name must be an integer."
    }
}

function Assert-JsonTrue {
    param(
        [AllowNull()] $Value,
        [Parameter(Mandatory = $true)][string] $Name
    )

    if ($Value -isnot [bool] -or -not $Value) {
        throw "$Name must be the JSON boolean true."
    }
}

function Convert-TexturePixelsToRgba8Bytes {
    param([Parameter(Mandatory = $true)][long] $PixelCount)

    if (
        $PixelCount -lt 0 -or
        $PixelCount -gt [long]::MaxValue / $rgba8BytesPerPixel
    ) {
        throw "Texture pixel count cannot be represented as RGBA8 bytes: $PixelCount."
    }
    return [long] ($PixelCount * $rgba8BytesPerPixel)
}

function Convert-CanvasGraphics {
    [OutputType([pscustomobject])]
    param([Parameter(Mandatory = $true)] $Event)

    $requiredFields = @(
        'graphics_webgl_version',
        'graphics_max_texture_size_px',
        'graphics_max_renderbuffer_size_px',
        'graphics_max_texture_image_units',
        'graphics_tested_media_id',
        'graphics_tested_texture_width_px',
        'graphics_tested_texture_height_px',
        'graphics_context_loss_mechanism',
        'graphics_context_lost',
        'graphics_context_restored',
        'graphics_recovery_duration_ms',
        'graphics_restored_frame_latency_ms',
        'graphics_gl_error',
        'graphics_texture_backed',
        'graphics_decorative_texture_backed'
    )
    $eventFields = @($Event.PSObject.Properties.Name)
    $missingFields = @(
        $requiredFields |
            Where-Object { $_ -notin $eventFields }
    )
    if ($missingFields.Count -gt 0) {
        throw (
            "The Canvas graphics probe is missing structured fields: " +
            "$($missingFields -join ', ')."
        )
    }

    foreach ($field in @(
        'graphics_webgl_version',
        'graphics_max_texture_size_px',
        'graphics_max_renderbuffer_size_px',
        'graphics_max_texture_image_units',
        'graphics_tested_texture_width_px',
        'graphics_tested_texture_height_px',
        'graphics_gl_error'
    )) {
        Assert-JsonNumber -Value $Event.$field -Name $field -Integer
    }
    foreach ($field in @(
        'graphics_recovery_duration_ms',
        'graphics_restored_frame_latency_ms'
    )) {
        Assert-JsonNumber -Value $Event.$field -Name $field
    }
    foreach ($field in @(
        'graphics_context_lost',
        'graphics_context_restored',
        'graphics_texture_backed',
        'graphics_decorative_texture_backed'
    )) {
        Assert-JsonTrue -Value $Event.$field -Name $field
    }
    foreach ($field in @(
        'graphics_tested_media_id',
        'graphics_context_loss_mechanism'
    )) {
        if ($Event.$field -isnot [string]) {
            throw "$field must be an explicit JSON string."
        }
    }

    $webglVersion = [long] $Event.graphics_webgl_version
    $maxTextureSizePx = [long] $Event.graphics_max_texture_size_px
    $maxRenderbufferSizePx = [long] $Event.graphics_max_renderbuffer_size_px
    $maxTextureImageUnits = [long] $Event.graphics_max_texture_image_units
    $testedMediaId = [string] $Event.graphics_tested_media_id
    $testedTextureWidthPx = [long] $Event.graphics_tested_texture_width_px
    $testedTextureHeightPx = [long] $Event.graphics_tested_texture_height_px
    $contextLossMechanism = [string] $Event.graphics_context_loss_mechanism
    $recoveryDurationMs = [double] $Event.graphics_recovery_duration_ms
    $restoredFrameLatencyMs = [double] $Event.graphics_restored_frame_latency_ms
    $glError = [long] $Event.graphics_gl_error

    Assert-FiniteNonNegativeNumber `
        -Value $recoveryDurationMs `
        -Name 'graphics recovery duration'
    Assert-FiniteNonNegativeNumber `
        -Value $restoredFrameLatencyMs `
        -Name 'restored frame latency'

    if (
        $webglVersion -ne 2 -or
        $maxTextureSizePx -lt 1 -or
        $maxRenderbufferSizePx -lt 1 -or
        $maxTextureImageUnits -lt 1 -or
        $testedMediaId -ne $graphicsTestedMediaId -or
        $testedTextureWidthPx -ne $graphicsTestedTextureWidthPx -or
        $testedTextureHeightPx -ne $graphicsTestedTextureHeightPx -or
        $testedTextureWidthPx -gt $maxTextureSizePx -or
        $testedTextureHeightPx -gt $maxTextureSizePx -or
        $contextLossMechanism -ne $graphicsContextLossMechanism -or
        -not [bool]$Event.graphics_context_lost -or
        -not [bool]$Event.graphics_context_restored -or
        $glError -ne 0 -or
        -not [bool]$Event.graphics_texture_backed -or
        -not [bool]$Event.graphics_decorative_texture_backed
    ) {
        throw (
            "The Canvas graphics probe did not prove the exact WebGL2 " +
            "context-recovery and 1600 x 1200 real-texture gate."
        )
    }

    $testedTexturePixelCount = [long](
        $testedTextureWidthPx * $testedTextureHeightPx
    )
    return [pscustomobject][ordered]@{
        webglVersion = $webglVersion
        limits = [ordered]@{
            maxTextureSizePx = $maxTextureSizePx
            maxRenderbufferSizePx = $maxRenderbufferSizePx
            maxTextureImageUnits = $maxTextureImageUnits
        }
        testedTexture = [ordered]@{
            mediaId = $testedMediaId
            widthPx = $testedTextureWidthPx
            heightPx = $testedTextureHeightPx
            pixelCount = $testedTexturePixelCount
            estimatedRgba8Bytes = Convert-TexturePixelsToRgba8Bytes `
                -PixelCount $testedTexturePixelCount
        }
        contextRecovery = [ordered]@{
            mechanism = $contextLossMechanism
            contextLost = [bool] $Event.graphics_context_lost
            contextRestored = [bool] $Event.graphics_context_restored
            recoveryDurationMs = $recoveryDurationMs
            restoredFrameLatencyMs = $restoredFrameLatencyMs
            glError = $glError
            textureBacked = [bool] $Event.graphics_texture_backed
            decorativeTextureBacked =
                [bool] $Event.graphics_decorative_texture_backed
        }
    }
}

function Assert-CanvasBenchmarkEvent {
    param([Parameter(Mandatory = $true)] $Event)

    if (-not [bool]$Event.texture_backed) {
        throw 'At least one Canvas probe did not use a real Cache texture.'
    }
    if (
        -not [bool]$Event.decorative_texture_backed -or
        [string]$Event.decorative_media_id -ne $graphicsTestedMediaId
    ) {
        throw 'At least one Canvas probe did not use the real transparent Decorative Cache texture.'
    }
    if (
        [long]$Event.navigation_sheet_count -lt 100 -or
        [long]$Event.navigation_cycle_count -ne 10 -or
        [long]$Event.navigation_sample_count -ne 30 -or
        [long]$Event.navigation_max_resident_sheet_count -lt 1 -or
        [long]$Event.navigation_max_resident_sheet_count -ge
            [long]$Event.navigation_sheet_count -or
        [long]$Event.navigation_max_resident_texture_count -lt 1 -or
        [long]$Event.navigation_max_resident_texture_pixel_count -lt 1
    ) {
        throw 'The long-Album navigation benchmark returned invalid evidence.'
    }

    return Convert-CanvasGraphics -Event $Event
}

function Measure-CanvasTimingAggregate {
    param([Parameter(Mandatory = $true)][object[]] $Timings)

    $sampleCount = [long] (($Timings | Measure-Object sampleCount -Sum).Sum)
    $weightedMean = if ($sampleCount -gt 0) {
        (
            $Timings |
                ForEach-Object { $_.meanFrameMs * $_.sampleCount } |
                Measure-Object -Sum
        ).Sum / $sampleCount
    }
    else {
        $null
    }
    return [ordered]@{
        sampleCount = $sampleCount
        weightedMeanFrameMs = $weightedMean
        worstProjectP95FrameMs = [double](
            ($Timings | Measure-Object p95FrameMs -Maximum).Maximum
        )
        worstProjectP99FrameMs = [double](
            ($Timings | Measure-Object p99FrameMs -Maximum).Maximum
        )
        worstProjectMaxFrameMs = [double](
            ($Timings | Measure-Object maxFrameMs -Maximum).Maximum
        )
        framesOver16Ms = [long](
            ($Timings | Measure-Object framesOver16Ms -Sum).Sum
        )
        framesOver33Ms = [long](
            ($Timings | Measure-Object framesOver33Ms -Sum).Sum
        )
    }
}

function Assert-ComparableCanvasTargets {
    param(
        [Parameter(Mandatory = $true)] $Independent,
        [Parameter(Mandatory = $true)] $Multiwindow
    )

    $independentTargets = @{}
    foreach ($project in $Independent.canvas.projects) {
        $independentTargets[[string]$project.projectId] = [ordered]@{
            frameId = [string]$project.frameId
            decorativeMediaId = [string]$project.decorativeMediaId
            testedMediaId = [string]$project.graphics.testedTexture.mediaId
            testedTextureWidthPx = [long]$project.graphics.testedTexture.widthPx
            testedTextureHeightPx = [long]$project.graphics.testedTexture.heightPx
            contextLossMechanism =
                [string]$project.graphics.contextRecovery.mechanism
            sheetCount = [long]$project.navigation.sheetCount
            targetSheetIds = @($project.navigation.targetSheetIds)
        }
    }
    foreach ($project in $Multiwindow.canvas.projects) {
        $projectId = [string]$project.projectId
        $independentTarget = $independentTargets[$projectId]
        if (
            $null -eq $independentTarget -or
            $independentTarget.frameId -ne [string]$project.frameId -or
            $independentTarget.decorativeMediaId -ne
                [string]$project.decorativeMediaId -or
            $independentTarget.testedMediaId -ne
                [string]$project.graphics.testedTexture.mediaId -or
            $independentTarget.testedTextureWidthPx -ne
                [long]$project.graphics.testedTexture.widthPx -or
            $independentTarget.testedTextureHeightPx -ne
                [long]$project.graphics.testedTexture.heightPx -or
            $independentTarget.contextLossMechanism -ne
                [string]$project.graphics.contextRecovery.mechanism -or
            $independentTarget.sheetCount -ne [long]$project.navigation.sheetCount -or
            (
                @($independentTarget.targetSheetIds) -join '|'
            ) -ne (
                @($project.navigation.targetSheetIds) -join '|'
            )
        ) {
            throw (
                "Canvas target mismatch for project ${projectId}: " +
                "independent frame/texture=" +
                "$($independentTarget.frameId)/" +
                "$($independentTarget.testedMediaId), " +
                "multiwindow frame/texture=$($project.frameId)/" +
                "$($project.graphics.testedTexture.mediaId)."
            )
        }
    }
}

function Wait-ForTopologyBenchmark {
    param(
        [Parameter(Mandatory = $true)]
        [int[]] $RootProcessIds,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedProjectCount,
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [DateTimeOffset] $StartedAt,
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Stopwatch] $TopologyStopwatch,
        [Parameter(Mandatory = $true)]
        [string] $ExportGatePath
    )

    $exportGateOpened = $false
    $postProbeGpuMemory = $null
    $exportPrefixes = @(
        $RootProcessIds | ForEach-Object { "export-$($_)-" }
    )
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($PerformanceTimeoutSeconds)
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        foreach ($processId in $RootProcessIds) {
            if ($null -eq (Get-Process -Id $processId -ErrorAction SilentlyContinue)) {
                throw "Topology host $processId exited during the interaction benchmark."
            }
        }

        $events = @(Get-DesktopLogEventsSince -Since $StartedAt)
        $benchmarkFailure = @(
            $events |
                Where-Object {
                    $_.event -eq 'topology_benchmark_failed' -and
                    [int]$_.process_id -in $RootProcessIds
                } |
                Sort-Object timestamp |
                Select-Object -First 1
        )
        if ($benchmarkFailure.Count -gt 0) {
            throw (
                "Canvas benchmark failed for project " +
                "$($benchmarkFailure[0].project_id): " +
                "$($benchmarkFailure[0].reason)."
            )
        }
        $exportFailure = @(
            $events |
                Where-Object {
                    $candidate = $_
                    $candidate.event -eq 'export_failed' -and
                    $null -ne $candidate.operation_id -and
                    @(
                        $exportPrefixes |
                            Where-Object {
                                ([string]$candidate.operation_id).StartsWith($_)
                            }
                    ).Count -gt 0
                } |
                Sort-Object timestamp |
                Select-Object -First 1
        )
        if ($exportFailure.Count -gt 0) {
            throw (
                "Export benchmark failed at stage " +
                "$($exportFailure[0].stage)."
            )
        }

        $canvasReadyEvents = @(
            $events |
                Where-Object {
                    $_.event -eq 'canvas_benchmark_ready' -and
                    $_.topology -eq $Topology -and
                    [int]$_.process_id -in $RootProcessIds
                } |
                Group-Object project_id |
                ForEach-Object {
                    $_.Group |
                        Sort-Object timestamp |
                        Select-Object -First 1
                }
        )
        $canvasEvents = @(
            $events |
                Where-Object {
                    $_.event -eq 'canvas_benchmark_completed' -and
                    $_.topology -eq $Topology -and
                    [int]$_.process_id -in $RootProcessIds
                } |
                Group-Object project_id |
                ForEach-Object {
                    $_.Group |
                        Sort-Object timestamp |
                        Select-Object -First 1
                }
        )
        $exportEvent = @(
            $events |
                Where-Object {
                    $_.event -eq 'export_completed' -and
                    $_.project_id -eq 'project-spike-001' -and
                    [int]$_.process_id -in $RootProcessIds
                } |
                Sort-Object timestamp |
                Select-Object -First 1
        )
        if (
            -not $exportGateOpened -and
            $canvasReadyEvents.Count -eq $ExpectedProjectCount -and
            $canvasEvents.Count -eq $ExpectedProjectCount
        ) {
            foreach ($canvasEvent in $canvasEvents) {
                [void] (Assert-CanvasBenchmarkEvent -Event $canvasEvent)
            }
            $processTreeIds = @(
                Get-ProcessTreeIds -RootProcessIds $RootProcessIds
            )
            $gpuMemory = Get-GpuMemory -ProcessIds $processTreeIds
            if (-not $gpuMemory.available) {
                throw (
                    "The post-probe Windows GPU memory snapshot is " +
                    "unavailable: $($gpuMemory.reason)"
                )
            }
            if ([long]$gpuMemory.totalBytes -le 0) {
                throw 'The post-probe Windows GPU memory snapshot is empty.'
            }
            $postProbeGpuMemory = [ordered]@{
                kind = 'post_probe_snapshot'
                capturedAtUtc = [DateTime]::UtcNow.ToString('o')
                source = 'windows_gpu_process_memory_counters'
                isPeak = $false
                dedicatedBytes = [long]$gpuMemory.dedicatedBytes
                sharedBytes = [long]$gpuMemory.sharedBytes
                totalBytes = [long]$gpuMemory.totalBytes
            }
            Open-TopologyProbeGate -Path $ExportGatePath
            $exportGateOpened = $true
        }
        if (
            $canvasReadyEvents.Count -eq $ExpectedProjectCount -and
            $canvasEvents.Count -eq $ExpectedProjectCount -and
            $exportEvent.Count -eq 1
        ) {
            if ($null -eq $postProbeGpuMemory) {
                throw 'The Export gate opened without the post-probe GPU memory snapshot.'
            }
            if (
                [int]$exportEvent[0].dpi -ne 300 -or
                [long]$exportEvent[0].source_count -lt 2 -or
                [long]$exportEvent[0].source_bytes -lt 1 -or
                [long]$exportEvent[0].output_bytes -lt 1 -or
                [string]$exportEvent[0].output_sha256 -notmatch '^[0-9a-f]{64}$'
            ) {
                throw 'The real-image Export benchmark returned invalid evidence.'
            }

            $readyByProject = @{}
            foreach ($readyEvent in $canvasReadyEvents) {
                $readyByProject[[string]$readyEvent.project_id] = $readyEvent
            }
            $projects = @(
                $canvasEvents |
                    Sort-Object project_id |
                    ForEach-Object {
                        $readyEvent = $readyByProject[[string]$_.project_id]
                        if ($null -eq $readyEvent) {
                            throw "Canvas readiness is missing for project $($_.project_id)."
                        }
                        $graphics = Assert-CanvasBenchmarkEvent -Event $_
                        $residentTexturePixelCount = [long](
                            $_.navigation_max_resident_texture_pixel_count
                        )
                        [ordered]@{
                            projectId = $_.project_id
                            processId = [int]$_.process_id
                            windowLabel = $_.window_label
                            readyElapsedMs = [long][math]::Round(
                                (
                                    [DateTimeOffset]::Parse($readyEvent.timestamp) -
                                    $StartedAt
                                ).TotalMilliseconds
                            )
                            frameId = $_.frame_id
                            textureBacked = [bool]$_.texture_backed
                            decorativeMediaId = [string]$_.decorative_media_id
                            decorativeTextureBacked = [bool]$_.decorative_texture_backed
                            graphics = $graphics
                            pan = Convert-CanvasTiming -Event $_ -Prefix pan
                            zoom = Convert-CanvasTiming -Event $_ -Prefix zoom
                            navigation = [ordered]@{
                                sheetCount = [long]$_.navigation_sheet_count
                                cycleCount = [long]$_.navigation_cycle_count
                                targetSheetIds = @(
                                    [string]$_.navigation_first_sheet_id
                                    [string]$_.navigation_middle_sheet_id
                                    [string]$_.navigation_last_sheet_id
                                )
                                maxResidentSheetCount = [long]$_.navigation_max_resident_sheet_count
                                maxResidentTextureCount = [long]$_.navigation_max_resident_texture_count
                                maxResidentTexturePixelCount =
                                    $residentTexturePixelCount
                                estimatedResidentRgba8Bytes =
                                    Convert-TexturePixelsToRgba8Bytes `
                                        -PixelCount $residentTexturePixelCount
                                timings = Convert-CanvasTiming -Event $_ -Prefix navigation
                            }
                        }
                    }
            )
            $panTimings = @($projects | ForEach-Object { $_.pan })
            $zoomTimings = @($projects | ForEach-Object { $_.zoom })
            $navigationTimings = @(
                $projects | ForEach-Object { $_.navigation.timings }
            )
            $graphicsMeasurements = @(
                $projects | ForEach-Object { $_.graphics }
            )
            $recoveryMeasurements = @(
                $graphicsMeasurements |
                    ForEach-Object { $_.contextRecovery }
            )
            $maxResidentTexturePixelCount = [long](
                (
                    $projects |
                        ForEach-Object {
                            [long]$_.navigation.maxResidentTexturePixelCount
                        } |
                        Measure-Object -Maximum
                ).Maximum
            )
            $sumOfProjectMaxResidentTexturePixelCount = [long](
                (
                    $projects |
                        ForEach-Object {
                            [long]$_.navigation.maxResidentTexturePixelCount
                        } |
                        Measure-Object -Sum
                ).Sum
            )
            $estimatedSumOfProjectMaxResidentRgba8Bytes =
                Convert-TexturePixelsToRgba8Bytes `
                    -PixelCount $sumOfProjectMaxResidentTexturePixelCount
            return [ordered]@{
                completedElapsedMs = $TopologyStopwatch.ElapsedMilliseconds
                postProbeGpuMemory = $postProbeGpuMemory
                canvas = [ordered]@{
                    projectCount = $projects.Count
                    allProjectsReadyElapsedMs = [long](
                        (
                            $projects |
                                ForEach-Object { [long]$_.readyElapsedMs } |
                                Measure-Object -Maximum
                        ).Maximum
                    )
                    warmupFramesPerProject = 24
                    projects = $projects
                    aggregate = [ordered]@{
                        pan = Measure-CanvasTimingAggregate -Timings $panTimings
                        zoom = Measure-CanvasTimingAggregate -Timings $zoomTimings
                        navigation = Measure-CanvasTimingAggregate -Timings $navigationTimings
                        graphics = [ordered]@{
                            webglVersion =
                                [long]$graphicsMeasurements[0].webglVersion
                            minimumMaxTextureSizePx = [long](
                                (
                                    $graphicsMeasurements |
                                        ForEach-Object {
                                            [long]$_.limits.maxTextureSizePx
                                        } |
                                        Measure-Object -Minimum
                                ).Minimum
                            )
                            minimumMaxRenderbufferSizePx = [long](
                                (
                                    $graphicsMeasurements |
                                        ForEach-Object {
                                            [long]$_.limits.maxRenderbufferSizePx
                                        } |
                                        Measure-Object -Minimum
                                ).Minimum
                            )
                            minimumMaxTextureImageUnits = [long](
                                (
                                    $graphicsMeasurements |
                                        ForEach-Object {
                                            [long]$_.limits.maxTextureImageUnits
                                        } |
                                        Measure-Object -Minimum
                                ).Minimum
                            )
                            testedTexture = [ordered]@{
                                mediaId = $graphicsTestedMediaId
                                widthPx = $graphicsTestedTextureWidthPx
                                heightPx = $graphicsTestedTextureHeightPx
                                pixelCount = [long](
                                    $graphicsTestedTextureWidthPx *
                                        $graphicsTestedTextureHeightPx
                                )
                                estimatedRgba8Bytes =
                                    Convert-TexturePixelsToRgba8Bytes `
                                        -PixelCount (
                                            [long](
                                                $graphicsTestedTextureWidthPx *
                                                    $graphicsTestedTextureHeightPx
                                            )
                                        )
                            }
                            contextRecovery = [ordered]@{
                                mechanism = $graphicsContextLossMechanism
                                lostCount = @(
                                    $recoveryMeasurements |
                                        Where-Object { $_.contextLost }
                                ).Count
                                restoredCount = @(
                                    $recoveryMeasurements |
                                        Where-Object { $_.contextRestored }
                                ).Count
                                projectCount = $projects.Count
                                worstRecoveryDurationMs = [double](
                                    (
                                        $recoveryMeasurements |
                                            ForEach-Object {
                                                [double]$_.recoveryDurationMs
                                            } |
                                            Measure-Object -Maximum
                                    ).Maximum
                                )
                                worstRestoredFrameLatencyMs = [double](
                                    (
                                        $recoveryMeasurements |
                                            ForEach-Object {
                                                [double]$_.restoredFrameLatencyMs
                                            } |
                                            Measure-Object -Maximum
                                    ).Maximum
                                )
                                glError = 0
                            }
                        }
                        maxResidentSheetCount = [long](
                            (
                                $projects |
                                    ForEach-Object {
                                        [long]$_.navigation.maxResidentSheetCount
                                    } |
                                    Measure-Object -Maximum
                            ).Maximum
                        )
                        maxResidentTextureCount = [long](
                            (
                                $projects |
                                    ForEach-Object {
                                        [long]$_.navigation.maxResidentTextureCount
                                    } |
                                    Measure-Object -Maximum
                            ).Maximum
                        )
                        maxResidentTexturePixelCount =
                            $maxResidentTexturePixelCount
                        sumOfProjectMaxResidentTexturePixelCount =
                            $sumOfProjectMaxResidentTexturePixelCount
                        estimatedSumOfProjectMaxResidentRgba8Bytes =
                            $estimatedSumOfProjectMaxResidentRgba8Bytes
                    }
                }
                export = [ordered]@{
                    projectId = $exportEvent[0].project_id
                    processId = [int]$exportEvent[0].process_id
                    windowLabel = $exportEvent[0].window_label
                    elapsedMs = [long]$exportEvent[0].elapsed_ms
                    widthPx = [long]$exportEvent[0].width_px
                    heightPx = [long]$exportEvent[0].height_px
                    dpi = [long]$exportEvent[0].dpi
                    sourceCount = [long]$exportEvent[0].source_count
                    sourceBytes = [long]$exportEvent[0].source_bytes
                    outputBytes = [long]$exportEvent[0].output_bytes
                    outputSha256 = [string]$exportEvent[0].output_sha256
                }
            }
        }
        Start-Sleep -Milliseconds 100
    }
    throw (
        "Expected Canvas and Export evidence for $ExpectedProjectCount " +
        "projects within $PerformanceTimeoutSeconds seconds."
    )
}

function Get-ProcessTreeIds {
    param([Parameter(Mandatory = $true)][int[]] $RootProcessIds)

    $allProcesses = @(Get-CimInstance Win32_Process)
    $known = [System.Collections.Generic.HashSet[int]]::new()
    $queue = [System.Collections.Generic.Queue[int]]::new()
    foreach ($rootProcessId in $RootProcessIds) {
        [void] $known.Add($rootProcessId)
        $queue.Enqueue($rootProcessId)
    }

    while ($queue.Count -gt 0) {
        $parentId = $queue.Dequeue()
        foreach ($child in $allProcesses | Where-Object { $_.ParentProcessId -eq $parentId }) {
            if ($known.Add([int] $child.ProcessId)) {
                $queue.Enqueue([int] $child.ProcessId)
            }
        }
    }
    return @($known | Sort-Object)
}

function Get-GpuMemory {
    param([Parameter(Mandatory = $true)][int[]] $ProcessIds)

    try {
        $samples = (Get-Counter -Counter @(
            '\GPU Process Memory(*)\Dedicated Usage',
            '\GPU Process Memory(*)\Shared Usage'
        ) -ErrorAction Stop).CounterSamples
        $dedicated = 0.0
        $shared = 0.0
        foreach ($sample in $samples) {
            if ($sample.InstanceName -notmatch 'pid_(\d+)_') {
                continue
            }
            if ([int] $matches[1] -notin $ProcessIds) {
                continue
            }
            if ($sample.Path -like '*Dedicated Usage') {
                $dedicated += $sample.CookedValue
            }
            elseif ($sample.Path -like '*Shared Usage') {
                $shared += $sample.CookedValue
            }
        }
        $dedicatedBytes = [long] $dedicated
        $sharedBytes = [long] $shared
        return [ordered]@{
            available = $true
            dedicatedBytes = $dedicatedBytes
            sharedBytes = $sharedBytes
            totalBytes = [long] ($dedicatedBytes + $sharedBytes)
        }
    }
    catch {
        return [ordered]@{
            available = $false
            reason = $_.Exception.Message
        }
    }
}

function Measure-TopologyProcesses {
    param([Parameter(Mandatory = $true)][int[]] $RootProcessIds)

    $treeProcessIds = @(Get-ProcessTreeIds -RootProcessIds $RootProcessIds)
    $processes = @($treeProcessIds | ForEach-Object {
        Get-Process -Id $_ -ErrorAction SilentlyContinue
    })
    return [ordered]@{
        hostProcessCount = $RootProcessIds.Count
        rootPriorityClasses = @(
            $RootProcessIds | ForEach-Object {
                $rootProcess = Get-Process -Id $_ -ErrorAction Stop
                $rootProcess.PriorityClass.ToString()
            }
        )
        processTreeCount = $processes.Count
        workingSetBytes = [long] (($processes | Measure-Object WorkingSet64 -Sum).Sum)
        privateMemoryBytes = [long] (($processes | Measure-Object PrivateMemorySize64 -Sum).Sum)
        handleCount = [long] (($processes | Measure-Object HandleCount -Sum).Sum)
        threadCount = [long] (($processes | ForEach-Object { $_.Threads.Count } | Measure-Object -Sum).Sum)
        processTree = @($processes | Sort-Object Id | ForEach-Object {
            [ordered]@{
                processId = $_.Id
                name = $_.ProcessName
                workingSetBytes = $_.WorkingSet64
                privateMemoryBytes = $_.PrivateMemorySize64
            }
        })
        gpuMemory = Get-GpuMemory -ProcessIds $treeProcessIds
    }
}

function Get-HardwareInventory {
    $operatingSystem = Get-CimInstance Win32_OperatingSystem
    $computer = Get-CimInstance Win32_ComputerSystem
    return [ordered]@{
        operatingSystem = [ordered]@{
            caption = $operatingSystem.Caption
            version = $operatingSystem.Version
            buildNumber = $operatingSystem.BuildNumber
        }
        cpu = @(
            Get-CimInstance Win32_Processor |
                Select-Object -ExpandProperty Name
        )
        totalPhysicalMemoryBytes = [long] $computer.TotalPhysicalMemory
        gpu = @(
            Get-CimInstance Win32_VideoController | ForEach-Object {
                [ordered]@{
                    name = $_.Name
                    adapterRamBytes = [long] $_.AdapterRAM
                    driverVersion = $_.DriverVersion
                }
            }
        )
    }
}

function Format-Mebibytes {
    param([Parameter(Mandatory = $true)][long] $Bytes)

    return ($Bytes / 1MB).ToString(
        'N1',
        [System.Globalization.CultureInfo]::GetCultureInfo('pt-BR')
    )
}

function Write-TopologyMarkdownSummary {
    param(
        [Parameter(Mandatory = $true)]
        $Report,
        [Parameter(Mandatory = $true)]
        $Text,
        [Parameter(Mandatory = $true)]
        [string] $SummaryPath
    )

    $summary = $Text.summary
    $independent = $Report.alternatives.independentHosts
    $multiwindow = $Report.alternatives.multiwindowHost
    $collectedDate = ([DateTime] $Report.collectedAtUtc).ToString('yyyy-MM-dd')
    $yes = $summary.yes
    $no = $summary.no
    $workingTreeDirty = if ($Report.build.workingTreeDirty) { $yes } else { $no }
    $buildInputsDirty = if ($Report.build.buildInputsDirty) { $yes } else { $no }
    $checkoutMatches = if (
        $Report.build.currentBuildInputsMatchManifest
    ) {
        $yes
    }
    else {
        $no
    }
    $independentAfterCrash = if (
        $independent.forcedFailure.projectHost.otherHostSurvived
    ) {
        (
            "$($independent.forcedFailure.projectHost.remainingWindowCount) " +
            "($($summary.otherPreserved))"
        )
    }
    else {
        "$($independent.forcedFailure.projectHost.remainingWindowCount)"
    }
    $multiwindowAfterCrash = (
        "$($multiwindow.forcedFailure.projectHost.remainingWindowCount)"
    )
    $independentGlobalRestarted = if (
        $independent.forcedFailure.globalProcess.explicitRestart.pidChanged
    ) { $yes } else { $no }
    $multiwindowGlobalRestarted = if (
        $multiwindow.forcedFailure.globalProcess.explicitRestart.pidChanged
    ) { $yes } else { $no }
    $independentHostRestart = $independent.forcedFailure.projectHost.explicitRestart
    $multiwindowHostRestart = $multiwindow.forcedFailure.projectHost.explicitRestart
    $independentReopenedProjectCount = $independentHostRestart.reopen.observedProjects
    $multiwindowReopenedProjectCount = $multiwindowHostRestart.reopen.observedProjects
    $independentHostReopened = if (
        $independentReopenedProjectCount -eq 1
    ) { $yes } else { $no }
    $multiwindowHostReopened = if (
        $multiwindowReopenedProjectCount -eq 2
    ) { $yes } else { $no }
    $cacheRecovered = if (
        $Report.failureGate.imagingProcessor.cacheRecoveredAfterOneExplicitRestart
    ) { $yes } else { $no }
    $exportFailedSafely = if (
        $Report.failureGate.imagingProcessor.exportFailedSafelyUntilExplicitRetry
    ) { $yes } else { $no }
    $sameTopologyBuildCommit = if (
        $Report.failureGate.imagingProcessor.sameGitCommitAsTopologyBuild
    ) { $yes } else { $no }

    $markdown = @(
        '---'
        'status: current'
        'document: technical-research'
        'ticket: 01-plataforma-e-arquitetura'
        "date: $collectedDate"
        "updated: $collectedDate"
        '---'
        ''
        "# $($summary.title)"
        ''
        "$($summary.collected): ``$($Report.collectedAtUtc)``."
        "$($summary.executionOrder): ``$($Report.execution.order)``."
        "[$($summary.raw)]($([System.IO.Path]::GetFileName($OutputPath)))."
        ''
        "| $($summary.measure) | $($summary.independent) | $($summary.multiwindow) |"
        '|---|---:|---:|'
        "| $($summary.hosts) | $($independent.processes.hostProcessCount) | $($multiwindow.processes.hostProcessCount) |"
        "| $($summary.windows) | $($independent.ready.windows.Count) | $($multiwindow.ready.windows.Count) |"
        "| $($summary.processes) | $($independent.processes.processTreeCount) | $($multiwindow.processes.processTreeCount) |"
        "| $($summary.workingSet) | $(Format-Mebibytes $independent.processes.workingSetBytes) MiB | $(Format-Mebibytes $multiwindow.processes.workingSetBytes) MiB |"
        "| $($summary.privateMemory) | $(Format-Mebibytes $independent.processes.privateMemoryBytes) MiB | $(Format-Mebibytes $multiwindow.processes.privateMemoryBytes) MiB |"
        "| $($summary.gpuPostProbeDedicated) | $(Format-Mebibytes $independent.interaction.postProbeGpuMemory.dedicatedBytes) MiB | $(Format-Mebibytes $multiwindow.interaction.postProbeGpuMemory.dedicatedBytes) MiB |"
        "| $($summary.gpuPostProbeShared) | $(Format-Mebibytes $independent.interaction.postProbeGpuMemory.sharedBytes) MiB | $(Format-Mebibytes $multiwindow.interaction.postProbeGpuMemory.sharedBytes) MiB |"
        "| $($summary.gpuPostProbeTotal) | $(Format-Mebibytes $independent.interaction.postProbeGpuMemory.totalBytes) MiB | $(Format-Mebibytes $multiwindow.interaction.postProbeGpuMemory.totalBytes) MiB |"
        "| $($summary.twoWindows) | $($independent.ready.elapsedMs) ms | $($multiwindow.ready.elapsedMs) ms |"
        "| $($summary.cacheReadyTime) | $($independent.cache.readyElapsedMs) ms | $($multiwindow.cache.readyElapsedMs) ms |"
        "| $($summary.canvasReadyTime) | $($independent.interaction.canvas.allProjectsReadyElapsedMs) ms | $($multiwindow.interaction.canvas.allProjectsReadyElapsedMs) ms |"
        "| $($summary.cacheWallTime) | $($independent.cache.cacheWallTimeMs) ms | $($multiwindow.cache.cacheWallTimeMs) ms |"
        "| $($summary.cachePhotos) | $($independent.cache.photoCount) | $($multiwindow.cache.photoCount) |"
        "| $($summary.cacheDecoratives) | $($independent.cache.decorativeCount) | $($multiwindow.cache.decorativeCount) |"
        "| $($summary.cacheThroughput) | $(Format-Mebibytes $independent.cache.sourceBytesPerSecond) MiB/s | $(Format-Mebibytes $multiwindow.cache.sourceBytesPerSecond) MiB/s |"
        "| $($summary.cacheSize) | $(Format-Mebibytes $independent.cache.previewBytes) MiB | $(Format-Mebibytes $multiwindow.cache.previewBytes) MiB |"
        "| $($summary.panP95) | $($independent.interaction.canvas.aggregate.pan.worstProjectP95FrameMs) ms | $($multiwindow.interaction.canvas.aggregate.pan.worstProjectP95FrameMs) ms |"
        "| $($summary.panOver33) | $($independent.interaction.canvas.aggregate.pan.framesOver33Ms) | $($multiwindow.interaction.canvas.aggregate.pan.framesOver33Ms) |"
        "| $($summary.zoomP95) | $($independent.interaction.canvas.aggregate.zoom.worstProjectP95FrameMs) ms | $($multiwindow.interaction.canvas.aggregate.zoom.worstProjectP95FrameMs) ms |"
        "| $($summary.zoomOver33) | $($independent.interaction.canvas.aggregate.zoom.framesOver33Ms) | $($multiwindow.interaction.canvas.aggregate.zoom.framesOver33Ms) |"
        "| $($summary.webglVersion) | $($independent.interaction.canvas.aggregate.graphics.webglVersion) | $($multiwindow.interaction.canvas.aggregate.graphics.webglVersion) |"
        "| $($summary.maxTextureSize) | $($independent.interaction.canvas.aggregate.graphics.minimumMaxTextureSizePx) px | $($multiwindow.interaction.canvas.aggregate.graphics.minimumMaxTextureSizePx) px |"
        "| $($summary.maxRenderbufferSize) | $($independent.interaction.canvas.aggregate.graphics.minimumMaxRenderbufferSizePx) px | $($multiwindow.interaction.canvas.aggregate.graphics.minimumMaxRenderbufferSizePx) px |"
        "| $($summary.maxTextureImageUnits) | $($independent.interaction.canvas.aggregate.graphics.minimumMaxTextureImageUnits) | $($multiwindow.interaction.canvas.aggregate.graphics.minimumMaxTextureImageUnits) |"
        "| $($summary.testedTexture) | $($independent.interaction.canvas.aggregate.graphics.testedTexture.widthPx) x $($independent.interaction.canvas.aggregate.graphics.testedTexture.heightPx) px | $($multiwindow.interaction.canvas.aggregate.graphics.testedTexture.widthPx) x $($multiwindow.interaction.canvas.aggregate.graphics.testedTexture.heightPx) px |"
        "| $($summary.contextRecovery) | $($independent.interaction.canvas.aggregate.graphics.contextRecovery.lostCount) perdidos / $($independent.interaction.canvas.aggregate.graphics.contextRecovery.restoredCount) restaurados | $($multiwindow.interaction.canvas.aggregate.graphics.contextRecovery.lostCount) perdidos / $($multiwindow.interaction.canvas.aggregate.graphics.contextRecovery.restoredCount) restaurados |"
        "| $($summary.recoveryDuration) | $($independent.interaction.canvas.aggregate.graphics.contextRecovery.worstRecoveryDurationMs) ms | $($multiwindow.interaction.canvas.aggregate.graphics.contextRecovery.worstRecoveryDurationMs) ms |"
        "| $($summary.restoredFrameLatency) | $($independent.interaction.canvas.aggregate.graphics.contextRecovery.worstRestoredFrameLatencyMs) ms | $($multiwindow.interaction.canvas.aggregate.graphics.contextRecovery.worstRestoredFrameLatencyMs) ms |"
        "| $($summary.navigationP95) | $($independent.interaction.canvas.aggregate.navigation.worstProjectP95FrameMs) ms | $($multiwindow.interaction.canvas.aggregate.navigation.worstProjectP95FrameMs) ms |"
        "| $($summary.navigationOver33) | $($independent.interaction.canvas.aggregate.navigation.framesOver33Ms) | $($multiwindow.interaction.canvas.aggregate.navigation.framesOver33Ms) |"
        "| $($summary.navigationResidentSheets) | $($independent.interaction.canvas.aggregate.maxResidentSheetCount) | $($multiwindow.interaction.canvas.aggregate.maxResidentSheetCount) |"
        "| $($summary.navigationResidentTextures) | $($independent.interaction.canvas.aggregate.maxResidentTextureCount) | $($multiwindow.interaction.canvas.aggregate.maxResidentTextureCount) |"
        "| $($summary.navigationResidentTexturePixels) | $($independent.interaction.canvas.aggregate.sumOfProjectMaxResidentTexturePixelCount) | $($multiwindow.interaction.canvas.aggregate.sumOfProjectMaxResidentTexturePixelCount) |"
        "| $($summary.navigationResidentTextureRgba8) | $(Format-Mebibytes $independent.interaction.canvas.aggregate.estimatedSumOfProjectMaxResidentRgba8Bytes) MiB | $(Format-Mebibytes $multiwindow.interaction.canvas.aggregate.estimatedSumOfProjectMaxResidentRgba8Bytes) MiB |"
        "| $($summary.exportDuration) | $($independent.interaction.export.elapsedMs) ms | $($multiwindow.interaction.export.elapsedMs) ms |"
        "| $($summary.exportDimensions) | $($independent.interaction.export.widthPx) x $($independent.interaction.export.heightPx) px | $($multiwindow.interaction.export.widthPx) x $($multiwindow.interaction.export.heightPx) px |"
        "| $($summary.exportSources) | $(Format-Mebibytes $independent.interaction.export.sourceBytes) MiB | $(Format-Mebibytes $multiwindow.interaction.export.sourceBytes) MiB |"
        "| $($summary.exportOutput) | $(Format-Mebibytes $independent.interaction.export.outputBytes) MiB | $(Format-Mebibytes $multiwindow.interaction.export.outputBytes) MiB |"
        "| $($summary.afterCrash) | $independentAfterCrash | $multiwindowAfterCrash |"
        ''
        "## $($summary.failureSection)"
        ''
        "| $($summary.evidence) | $($summary.independent) | $($summary.multiwindow) |"
        '|---|---:|---:|'
        "| $($summary.globalOwnWindows) | $($independent.forcedFailure.globalProcess.initial.visibleWindowCount) | $($multiwindow.forcedFailure.globalProcess.initial.visibleWindowCount) |"
        "| $($summary.globalWorkingSet) | $(Format-Mebibytes $independent.forcedFailure.globalProcess.initial.processes.workingSetBytes) MiB | $(Format-Mebibytes $multiwindow.forcedFailure.globalProcess.initial.processes.workingSetBytes) MiB |"
        "| $($summary.windowsWhileGlobalUnavailable) | $($independent.forcedFailure.globalProcess.windowsWhileUnavailable.observedCount) | $($multiwindow.forcedFailure.globalProcess.windowsWhileUnavailable.observedCount) |"
        "| $($summary.projectsWhileGlobalUnavailable) | $($independent.forcedFailure.globalProcess.offlineContinuity.observedCompletions) | $($multiwindow.forcedFailure.globalProcess.offlineContinuity.observedCompletions) |"
        "| $($summary.globalExplicitRestart) | $independentGlobalRestarted | $multiwindowGlobalRestarted |"
        "| $($summary.projectsAfterGlobalRestart) | $($independent.forcedFailure.globalProcess.onlineContinuity.observedCompletions) | $($multiwindow.forcedFailure.globalProcess.onlineContinuity.observedCompletions) |"
        "| $($summary.windowsAfterHostFailure) | $independentAfterCrash | $multiwindowAfterCrash |"
        "| $($summary.savedRevisionReopened) | $independentHostReopened | $multiwindowHostReopened |"
        "| $($summary.interruptedGlobalLinks) | $($independent.forcedFailure.ipc.linksInterruptedByGlobalCrash) | $($multiwindow.forcedFailure.ipc.linksInterruptedByGlobalCrash) |"
        "| $($summary.minimumHostCommands) | $($independent.forcedFailure.ipc.minimumProjectHostCommandsPerProjectProbe) | $($multiwindow.forcedFailure.ipc.minimumProjectHostCommandsPerProjectProbe) |"
        "| $($summary.minimumCorrelatedInteractions) | $($independent.forcedFailure.ipc.minimumCorrelatedInteractionsPerProjectProbe) | $($multiwindow.forcedFailure.ipc.minimumCorrelatedInteractionsPerProjectProbe) |"
        "| $($summary.probeFailureEvents) | $($independent.forcedFailure.logs.projectHosts.continuityFailureEvents) | $($multiwindow.forcedFailure.logs.projectHosts.continuityFailureEvents) |"
        ''
        "$($summary.ipcExplanation)"
        ''
        "## $($summary.imagingSection)"
        ''
        "- $($summary.validatedArtifact): ``$($Report.failureGate.imagingProcessor.artifact)``."
        "- $($summary.artifactSha256): ``$($Report.failureGate.imagingProcessor.artifactSha256)``."
        "- $($summary.cacheRecovered): $cacheRecovered."
        "- $($summary.exportFailedSafely): $exportFailedSafely."
        "- $($summary.sameTopologyBuildCommit): $sameTopologyBuildCommit."
        ''
        "## $($summary.corpus)"
        ''
        "- $($summary.corpusAlbums): $($Report.corpus.albumCount)"
        "- $($summary.corpusPhotos): $($Report.corpus.photoCount)"
        "- $($summary.corpusDecoratives): $($Report.corpus.decorativeCount)"
        "- $($summary.corpusSourceVolume): $(Format-Mebibytes $Report.corpus.sourceBytes) MiB"
        "- $($summary.corpusDigest): ``$($Report.corpus.corpusSha256)``"
        "- $($summary.corpusIntegrity): $($summary.corpusIntegrityValue)"
        "- $($summary.previewPolicy): $($summary.previewPolicyValue -f $Report.corpus.previewPolicy.maximumEdgePx)"
        ''
        "## $($summary.build)"
        ''
        "- $($summary.commit): ``$($Report.build.gitCommit)``"
        "- $($summary.builtAt): ``$($Report.build.builtAtUtc)``"
        "- $($summary.profile): ``$($Report.build.profile)``"
        "- $($summary.workingTreeDirty): $workingTreeDirty"
        "- $($summary.buildInputsDirty): $buildInputsDirty"
        "- $($summary.buildInputCount): $($Report.build.buildInputFileCount)"
        "- $($summary.buildInputDigest): ``$($Report.build.buildInputDigestSha256)``"
        "- $($summary.executableHash): ``$($Report.build.executableSha256)``"
        "- $($summary.imagingExecutableHash): ``$($Report.build.imagingExecutableSha256)``"
        "- $($summary.checkoutMatches): $checkoutMatches"
        ''
        "## $($summary.environment)"
        ''
        "- $($summary.operatingSystem): $($Report.hardware.operatingSystem.caption) ``$($Report.hardware.operatingSystem.version)``"
        "- $($summary.cpu): $($Report.hardware.cpu -join '; ')"
        "- $($summary.physicalMemory): $(Format-Mebibytes $Report.hardware.totalPhysicalMemoryBytes) MiB"
        ''
        "## $($summary.notMeasured)"
        ''
    )
    $markdown += @($Report.notMeasured | ForEach-Object { "- $_" })
    $markdown += @(
        ''
        "## $($summary.notes)"
        ''
    )
    $markdown += @($Report.notes | ForEach-Object { "- $_" })

    $markdownText = ($markdown -join [System.Environment]::NewLine) +
        [System.Environment]::NewLine
    $mojibakeSequences = @(
        ([char] 0x00c2).ToString()
        ([char] 0x00c3).ToString()
        (([char] 0x00e2).ToString() + [char] 0x20ac)
        (([char] 0x00e2).ToString() + [char] 0x2020)
    )
    foreach ($sequence in $mojibakeSequences) {
        if ($markdownText.IndexOf($sequence) -ge 0) {
            throw 'The generated Markdown summary contains mojibake.'
        }
    }
    foreach ($requiredText in @(
        $summary.evidence,
        $summary.globalExplicitRestart,
        $summary.imagingSection
    )) {
        if (-not $markdownText.Contains($requiredText)) {
            throw 'The generated Markdown summary lost localized text.'
        }
    }

    [System.IO.File]::WriteAllText(
        $SummaryPath,
        $markdownText,
        [System.Text.UTF8Encoding]::new($false)
    )
}

$previousCargoTarget = [System.Environment]::GetEnvironmentVariable(
    'CARGO_TARGET_DIR',
    [System.EnvironmentVariableTarget]::Process
)
$failureScratchRoot = $null

try {
    & (Join-Path $PSScriptRoot 'Prepare-TopologyCorpus.ps1')
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    $corpusManifest = Get-Content `
        -LiteralPath $corpusManifestPath `
        -Raw `
        -Encoding utf8 |
            ConvertFrom-Json

    if (-not $SkipBuild) {
        Set-ProcessEnvironmentValue -Name 'CARGO_TARGET_DIR' -Value $targetDirectory
        & (Join-Path $PSScriptRoot 'Invoke-LocalTauri.ps1') build --no-bundle
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
        $buildManifest = New-TopologyBuildManifest
    }
    else {
        $buildManifest = Read-TopologyBuildManifest
    }

    $runId = "$PID-$([DateTime]::UtcNow.Ticks)"
    $failureScratchRoot = Join-Path `
        (Join-Path $script:WorkspaceRoot '.scratch') `
        "topology-failure-$runId"
    $independentFaultOutputRoot = Join-Path `
        $failureScratchRoot `
        'independent'
    $multiwindowFaultOutputRoot = Join-Path `
        $failureScratchRoot `
        'multiwindow'
    New-Item `
        -ItemType Directory `
        -Force `
        -Path $independentFaultOutputRoot, $multiwindowFaultOutputRoot |
            Out-Null

    $independentProbeGate = Join-Path `
        $probeGateDirectory `
        "independent-$runId.ready"
    $multiwindowProbeGate = Join-Path `
        $probeGateDirectory `
        "multiwindow-$runId.ready"
    $independentExportGate = Join-Path `
        $probeGateDirectory `
        "independent-export-$runId.ready"
    $multiwindowExportGate = Join-Path `
        $probeGateDirectory `
        "multiwindow-export-$runId.ready"
    $independentFaultGate = Join-Path `
        $probeGateDirectory `
        "independent-fault-$runId.json"
    $multiwindowFaultGate = Join-Path `
        $probeGateDirectory `
        "multiwindow-fault-$runId.json"
    $probeGatePaths.Add($independentProbeGate)
    $probeGatePaths.Add($multiwindowProbeGate)
    $probeGatePaths.Add($independentExportGate)
    $probeGatePaths.Add($multiwindowExportGate)
    $probeGatePaths.Add($independentFaultGate)
    $probeGatePaths.Add($multiwindowFaultGate)

    $imagingRecovery = Read-ValidatedImagingRecoveryArtifact `
        -Path $ImagingRecoveryPath `
        -TopologyBuildCommit $buildManifest.gitCommit

    # These seams are dot-sourced below so their named evidence remains in
    # the report scope without duplicating the topology-specific rules.
    $runIndependentTopology = {
    Reset-TopologyCache
    $independentStartedAt = [DateTimeOffset]::UtcNow
    $independentStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $independentEndpoint = Get-AvailableTopologyEndpoint
    $independentGlobal = Start-TopologyGlobalProcess `
        -Topology independent `
        -RunId $runId `
        -Endpoint $independentEndpoint
    $independentGlobalStatus = Wait-ForTopologyGlobalStatus `
        -Process $independentGlobal `
        -Topology independent `
        -Endpoint $independentEndpoint `
        -RunId $runId `
        -ProbeId 'independent-global-initial'
    $independentGlobalSingleton = Confirm-TopologyGlobalSingleton `
        -Topology independent `
        -RunId $runId `
        -Endpoint $independentEndpoint `
        -OwnerProcessId $independentGlobal.Id `
        -ProbeId 'independent-singleton-initial'
    $independentGlobalMetrics = Measure-TopologyProcesses `
        -RootProcessIds @($independentGlobal.Id)
    $independentGlobalWindowCount = (
        [MyAlbunsWindowProbe]::VisibleWindowsFor(
            @($independentGlobal.Id)
        ).Count
    )
    if ($independentGlobalWindowCount -ne 0) {
        throw 'The lightweight global role unexpectedly owns a visible window.'
    }

    $independentA = Start-TopologyProcess `
        -Topology independent `
        -ProjectSlot a `
        -ProbeGatePath $independentProbeGate `
        -ExportGatePath $independentExportGate `
        -RunId $runId `
        -GlobalEndpoint $independentEndpoint `
        -FaultGatePath $independentFaultGate `
        -FaultOutputRoot $independentFaultOutputRoot
    $independentB = Start-TopologyProcess `
        -Topology independent `
        -ProjectSlot b `
        -ProbeGatePath $independentProbeGate `
        -ExportGatePath $independentExportGate `
        -RunId $runId `
        -GlobalEndpoint $independentEndpoint `
        -FaultGatePath $independentFaultGate `
        -FaultOutputRoot $independentFaultOutputRoot
    $independentReady = Wait-ForTopologyWindows `
        -RootProcessIds @($independentA.Id, $independentB.Id) `
        -ExpectedCount 2 `
        -ExpectedTitleMarker '[Topologia A]' `
        -Stopwatch $independentStopwatch
    $independentCache = Wait-ForMediaCache `
        -RootProcessIds @($independentA.Id, $independentB.Id) `
        -ExpectedProjectCount 2 `
        -StartedAt $independentStartedAt `
        -TopologyStopwatch $independentStopwatch
    Open-TopologyProbeGate -Path $independentProbeGate
    $independentInteraction = Wait-ForTopologyBenchmark `
        -RootProcessIds @($independentA.Id, $independentB.Id) `
        -ExpectedProjectCount 2 `
        -Topology independent `
        -StartedAt $independentStartedAt `
        -TopologyStopwatch $independentStopwatch `
        -ExportGatePath $independentExportGate
    $independentMetrics = Measure-TopologyProcesses `
        -RootProcessIds @($independentA.Id, $independentB.Id)

    $independentGlobalCrash = Invoke-TopologyProcessCrash `
        -ProcessId $independentGlobal.Id `
        -Role 'global_shell'
    Start-Sleep -Milliseconds 750
    $independentGlobalUnavailable = Confirm-TopologyGlobalUnavailable `
        -Endpoint $independentEndpoint `
        -RunId $runId `
        -ProbeId 'independent-global-down-runner'
    $independentProcessesWhileGlobalUnavailable = (
        Assert-TopologyExecutableProcessSet `
            -ExpectedProcessIds @($independentA.Id, $independentB.Id) `
            -Context 'Independent process set after the global crash'
    )
    $independentWindowsWhileGlobalUnavailable = Assert-TopologyWindowCount `
        -RootProcessIds @($independentA.Id, $independentB.Id) `
        -ExpectedCount 2 `
        -Context 'Independent hosts after the global crash'
    $independentOfflineProbe = Invoke-TopologyContinuityProbe `
        -Topology independent `
        -RunId $runId `
        -ProbeId 'independent-global-down' `
        -ExpectedGlobalAvailable $false `
        -RootProcessIds @($independentA.Id, $independentB.Id) `
        -ExpectedProjectCount 2 `
        -GatePath $independentFaultGate `
        -OutputRoot $independentFaultOutputRoot `
        -TopologyStartedAt $independentStartedAt

    $independentRestartedGlobal = Start-TopologyGlobalProcess `
        -Topology independent `
        -RunId $runId `
        -Endpoint $independentEndpoint
    $independentRestartedGlobalStatus = Wait-ForTopologyGlobalStatus `
        -Process $independentRestartedGlobal `
        -Topology independent `
        -Endpoint $independentEndpoint `
        -RunId $runId `
        -ProbeId 'independent-global-restarted'
    $independentRestartedSingleton = Confirm-TopologyGlobalSingleton `
        -Topology independent `
        -RunId $runId `
        -Endpoint $independentEndpoint `
        -OwnerProcessId $independentRestartedGlobal.Id `
        -ProbeId 'independent-singleton-restarted'
    $independentOnlineProbe = Invoke-TopologyContinuityProbe `
        -Topology independent `
        -RunId $runId `
        -ProbeId 'independent-global-restored' `
        -ExpectedGlobalAvailable $true `
        -RootProcessIds @($independentA.Id, $independentB.Id) `
        -ExpectedProjectCount 2 `
        -GatePath $independentFaultGate `
        -OutputRoot $independentFaultOutputRoot `
        -TopologyStartedAt $independentStartedAt
    Assert-TopologyProbeGlobalOwner `
        -Probe $independentOnlineProbe `
        -ExpectedProcessId $independentRestartedGlobal.Id

    $independentHostCrash = Invoke-TopologyProcessCrash `
        -ProcessId $independentA.Id `
        -Role 'project_host_a'
    Start-Sleep -Milliseconds 750
    $independentSurvivingWindows = Assert-TopologyWindowCount `
        -RootProcessIds @($independentB.Id) `
        -ExpectedCount 1 `
        -Context 'Independent peer after one Project host crash'
    $independentProcessesAfterHostCrash = (
        Assert-TopologyExecutableProcessSet `
            -ExpectedProcessIds @(
                $independentB.Id,
                $independentRestartedGlobal.Id
            ) `
            -Context 'Independent process set after one host crash'
    )
    $independentSurvivorProbe = Invoke-TopologyContinuityProbe `
        -Topology independent `
        -RunId $runId `
        -ProbeId 'independent-peer-host-down' `
        -ExpectedGlobalAvailable $true `
        -RootProcessIds @($independentB.Id) `
        -ExpectedProjectCount 1 `
        -GatePath $independentFaultGate `
        -OutputRoot $independentFaultOutputRoot `
        -TopologyStartedAt $independentStartedAt
    Assert-TopologyProbeGlobalOwner `
        -Probe $independentSurvivorProbe `
        -ExpectedProcessId $independentRestartedGlobal.Id

    $independentProjectA = Get-TopologyProbeProject `
        -Probe $independentOnlineProbe `
        -ProjectId 'project-spike-001'
    $independentProjectASource = Get-TopologyProbeSource `
        -Probe $independentOnlineProbe `
        -ProjectId 'project-spike-001'
    Remove-TopologyFaultGate -Path $independentProbeGate
    Remove-TopologyFaultGate -Path $independentExportGate
    $independentHostRestartStartedAt = [DateTimeOffset]::UtcNow
    $independentHostRestartStopwatch = (
        [System.Diagnostics.Stopwatch]::StartNew()
    )
    $independentRestartedA = Start-TopologyProcess `
        -Topology independent `
        -ProjectSlot a `
        -ProbeGatePath $independentProbeGate `
        -ExportGatePath $independentExportGate `
        -RunId $runId `
        -GlobalEndpoint $independentEndpoint `
        -FaultGatePath $independentFaultGate `
        -FaultOutputRoot $independentFaultOutputRoot `
        -ProjectASource $independentProjectASource
    $independentRestartedWindows = Wait-ForTopologyWindows `
        -RootProcessIds @($independentRestartedA.Id, $independentB.Id) `
        -ExpectedCount 2 `
        -ExpectedTitleMarker '[Topologia A]' `
        -Stopwatch $independentHostRestartStopwatch
    $independentRestartedCache = Wait-ForMediaCache `
        -RootProcessIds @($independentRestartedA.Id) `
        -ExpectedProjectCount 1 `
        -StartedAt $independentHostRestartStartedAt `
        -TopologyStopwatch $independentHostRestartStopwatch
    $independentReopen = Wait-ForTopologyProjectReopen `
        -Topology independent `
        -RunId $runId `
        -ExpectedProjects @($independentProjectA) `
        -RootProcessIds @($independentRestartedA.Id) `
        -StartedAt $independentHostRestartStartedAt
    $independentGlobalAfterHostRestart = Invoke-TopologyGlobalStatus `
        -Endpoint $independentEndpoint `
        -RunId $runId `
        -ProbeId 'independent-after-host-restart'
    if (
        $independentGlobalAfterHostRestart.processId -ne
            $independentRestartedGlobal.Id
    ) {
        throw 'The independent host restart changed the global owner.'
    }
    $independentLogQuality = Wait-ForTopologyFailureLogQuality `
        -Topology independent `
        -RunId $runId `
        -StartedAt $independentStartedAt `
        -ExpectedHostStreamCount 3 `
        -ExpectedCompletionCount 5 `
        -ExpectedReopenCount 1
    $independentFailureIsolation = [ordered]@{
        globalProcess = [ordered]@{
            initial = [ordered]@{
                processId = $independentGlobal.Id
                status = $independentGlobalStatus
                singleton = $independentGlobalSingleton
                visibleWindowCount = $independentGlobalWindowCount
                processes = $independentGlobalMetrics
            }
            termination = $independentGlobalCrash
            unavailableBeforeExplicitRestart = $independentGlobalUnavailable
            noAutomaticRestartObserved = (
                $independentProcessesWhileGlobalUnavailable.
                    unexpectedProcessIds.Count -eq 0
            )
            processSetWhileUnavailable = (
                $independentProcessesWhileGlobalUnavailable
            )
            windowsWhileUnavailable = (
                $independentWindowsWhileGlobalUnavailable
            )
            offlineContinuity = (
                Select-TopologyProbeReport -Probe $independentOfflineProbe
            )
            explicitRestart = [ordered]@{
                previousProcessId = $independentGlobal.Id
                processId = $independentRestartedGlobal.Id
                pidChanged = (
                    $independentGlobal.Id -ne $independentRestartedGlobal.Id
                )
                status = $independentRestartedGlobalStatus
                singleton = $independentRestartedSingleton
            }
            onlineContinuity = (
                Select-TopologyProbeReport -Probe $independentOnlineProbe
            )
        }
        projectHost = [ordered]@{
            termination = $independentHostCrash
            forcedHostProcessId = $independentA.Id
            hostSurvived = $false
            otherHostSurvived = $null -ne (
                Get-Process `
                    -Id $independentB.Id `
                    -ErrorAction SilentlyContinue
            )
            noAutomaticRestartObserved = $null -eq (
                Get-Process `
                    -Id $independentA.Id `
                    -ErrorAction SilentlyContinue
            )
            processSetAfterCrash = $independentProcessesAfterHostCrash
            remainingWindowCount = (
                $independentSurvivingWindows.observedCount
            )
            survivorContinuity = (
                Select-TopologyProbeReport -Probe $independentSurvivorProbe
            )
            explicitRestart = [ordered]@{
                previousProcessId = $independentA.Id
                processId = $independentRestartedA.Id
                pidChanged = (
                    $independentA.Id -ne $independentRestartedA.Id
                )
                ready = $independentRestartedWindows
                cacheReady = $independentRestartedCache
                reopen = $independentReopen
                globalStatus = $independentGlobalAfterHostRestart
            }
        }
        ipc = New-TopologyIpcEvidence `
            -Topology independent `
            -SuccessfulProjectProbeCount 5
        logs = $independentLogQuality
    }
    Stop-OwnedTopologyProcess -ProcessId $independentRestartedA.Id
    Stop-OwnedTopologyProcess -ProcessId $independentB.Id
    Stop-OwnedTopologyProcess -ProcessId $independentRestartedGlobal.Id
    }

    $runMultiwindowTopology = {
    Reset-TopologyCache
    $multiwindowStartedAt = [DateTimeOffset]::UtcNow
    $multiwindowStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $multiwindowEndpoint = Get-AvailableTopologyEndpoint
    $multiwindowGlobal = Start-TopologyGlobalProcess `
        -Topology multiwindow `
        -RunId $runId `
        -Endpoint $multiwindowEndpoint
    $multiwindowGlobalStatus = Wait-ForTopologyGlobalStatus `
        -Process $multiwindowGlobal `
        -Topology multiwindow `
        -Endpoint $multiwindowEndpoint `
        -RunId $runId `
        -ProbeId 'multiwindow-global-initial'
    $multiwindowGlobalSingleton = Confirm-TopologyGlobalSingleton `
        -Topology multiwindow `
        -RunId $runId `
        -Endpoint $multiwindowEndpoint `
        -OwnerProcessId $multiwindowGlobal.Id `
        -ProbeId 'multiwindow-singleton-initial'
    $multiwindowGlobalMetrics = Measure-TopologyProcesses `
        -RootProcessIds @($multiwindowGlobal.Id)
    $multiwindowGlobalWindowCount = (
        [MyAlbunsWindowProbe]::VisibleWindowsFor(
            @($multiwindowGlobal.Id)
        ).Count
    )
    if ($multiwindowGlobalWindowCount -ne 0) {
        throw 'The lightweight global role unexpectedly owns a visible window.'
    }

    $multiwindow = Start-TopologyProcess `
        -Topology multiwindow `
        -ProbeGatePath $multiwindowProbeGate `
        -ExportGatePath $multiwindowExportGate `
        -RunId $runId `
        -GlobalEndpoint $multiwindowEndpoint `
        -FaultGatePath $multiwindowFaultGate `
        -FaultOutputRoot $multiwindowFaultOutputRoot
    $multiwindowReady = Wait-ForTopologyWindows `
        -RootProcessIds @($multiwindow.Id) `
        -ExpectedCount 2 `
        -ExpectedTitleMarker '[Topologia B]' `
        -Stopwatch $multiwindowStopwatch
    $multiwindowCache = Wait-ForMediaCache `
        -RootProcessIds @($multiwindow.Id) `
        -ExpectedProjectCount 2 `
        -StartedAt $multiwindowStartedAt `
        -TopologyStopwatch $multiwindowStopwatch
    Open-TopologyProbeGate -Path $multiwindowProbeGate
    $multiwindowInteraction = Wait-ForTopologyBenchmark `
        -RootProcessIds @($multiwindow.Id) `
        -ExpectedProjectCount 2 `
        -Topology multiwindow `
        -StartedAt $multiwindowStartedAt `
        -TopologyStopwatch $multiwindowStopwatch `
        -ExportGatePath $multiwindowExportGate
    $multiwindowMetrics = Measure-TopologyProcesses `
        -RootProcessIds @($multiwindow.Id)

    $multiwindowGlobalCrash = Invoke-TopologyProcessCrash `
        -ProcessId $multiwindowGlobal.Id `
        -Role 'global_shell'
    Start-Sleep -Milliseconds 750
    $multiwindowGlobalUnavailable = Confirm-TopologyGlobalUnavailable `
        -Endpoint $multiwindowEndpoint `
        -RunId $runId `
        -ProbeId 'multiwindow-global-down-runner'
    $multiwindowProcessesWhileGlobalUnavailable = (
        Assert-TopologyExecutableProcessSet `
            -ExpectedProcessIds @($multiwindow.Id) `
            -Context 'Multiwindow process set after the global crash'
    )
    $multiwindowWindowsWhileGlobalUnavailable = Assert-TopologyWindowCount `
        -RootProcessIds @($multiwindow.Id) `
        -ExpectedCount 2 `
        -Context 'Multiwindow host after the global crash'
    $multiwindowOfflineProbe = Invoke-TopologyContinuityProbe `
        -Topology multiwindow `
        -RunId $runId `
        -ProbeId 'multiwindow-global-down' `
        -ExpectedGlobalAvailable $false `
        -RootProcessIds @($multiwindow.Id) `
        -ExpectedProjectCount 2 `
        -GatePath $multiwindowFaultGate `
        -OutputRoot $multiwindowFaultOutputRoot `
        -TopologyStartedAt $multiwindowStartedAt

    $multiwindowRestartedGlobal = Start-TopologyGlobalProcess `
        -Topology multiwindow `
        -RunId $runId `
        -Endpoint $multiwindowEndpoint
    $multiwindowRestartedGlobalStatus = Wait-ForTopologyGlobalStatus `
        -Process $multiwindowRestartedGlobal `
        -Topology multiwindow `
        -Endpoint $multiwindowEndpoint `
        -RunId $runId `
        -ProbeId 'multiwindow-global-restarted'
    $multiwindowRestartedSingleton = Confirm-TopologyGlobalSingleton `
        -Topology multiwindow `
        -RunId $runId `
        -Endpoint $multiwindowEndpoint `
        -OwnerProcessId $multiwindowRestartedGlobal.Id `
        -ProbeId 'multiwindow-singleton-restarted'
    $multiwindowOnlineProbe = Invoke-TopologyContinuityProbe `
        -Topology multiwindow `
        -RunId $runId `
        -ProbeId 'multiwindow-global-restored' `
        -ExpectedGlobalAvailable $true `
        -RootProcessIds @($multiwindow.Id) `
        -ExpectedProjectCount 2 `
        -GatePath $multiwindowFaultGate `
        -OutputRoot $multiwindowFaultOutputRoot `
        -TopologyStartedAt $multiwindowStartedAt
    Assert-TopologyProbeGlobalOwner `
        -Probe $multiwindowOnlineProbe `
        -ExpectedProcessId $multiwindowRestartedGlobal.Id

    $multiwindowHostCrash = Invoke-TopologyProcessCrash `
        -ProcessId $multiwindow.Id `
        -Role 'project_host_multiwindow'
    Start-Sleep -Milliseconds 750
    $multiwindowStoppedWindows = Assert-TopologyWindowCount `
        -RootProcessIds @($multiwindow.Id) `
        -ExpectedCount 0 `
        -Context 'Multiwindow windows after the shared host crash'
    $multiwindowProcessesAfterHostCrash = (
        Assert-TopologyExecutableProcessSet `
            -ExpectedProcessIds @($multiwindowRestartedGlobal.Id) `
            -Context 'Multiwindow process set after the shared host crash'
    )
    $multiwindowProjectA = Get-TopologyProbeProject `
        -Probe $multiwindowOnlineProbe `
        -ProjectId 'project-spike-001'
    $multiwindowProjectB = Get-TopologyProbeProject `
        -Probe $multiwindowOnlineProbe `
        -ProjectId 'project-spike-002'
    $multiwindowProjectASource = Get-TopologyProbeSource `
        -Probe $multiwindowOnlineProbe `
        -ProjectId 'project-spike-001'
    $multiwindowProjectBSource = Get-TopologyProbeSource `
        -Probe $multiwindowOnlineProbe `
        -ProjectId 'project-spike-002'
    Remove-TopologyFaultGate -Path $multiwindowProbeGate
    Remove-TopologyFaultGate -Path $multiwindowExportGate
    $multiwindowHostRestartStartedAt = [DateTimeOffset]::UtcNow
    $multiwindowHostRestartStopwatch = (
        [System.Diagnostics.Stopwatch]::StartNew()
    )
    $multiwindowRestarted = Start-TopologyProcess `
        -Topology multiwindow `
        -ProbeGatePath $multiwindowProbeGate `
        -ExportGatePath $multiwindowExportGate `
        -RunId $runId `
        -GlobalEndpoint $multiwindowEndpoint `
        -FaultGatePath $multiwindowFaultGate `
        -FaultOutputRoot $multiwindowFaultOutputRoot `
        -ProjectASource $multiwindowProjectASource `
        -ProjectBSource $multiwindowProjectBSource
    $multiwindowRestartedWindows = Wait-ForTopologyWindows `
        -RootProcessIds @($multiwindowRestarted.Id) `
        -ExpectedCount 2 `
        -ExpectedTitleMarker '[Topologia B]' `
        -Stopwatch $multiwindowHostRestartStopwatch
    $multiwindowRestartedCache = Wait-ForMediaCache `
        -RootProcessIds @($multiwindowRestarted.Id) `
        -ExpectedProjectCount 2 `
        -StartedAt $multiwindowHostRestartStartedAt `
        -TopologyStopwatch $multiwindowHostRestartStopwatch
    $multiwindowReopen = Wait-ForTopologyProjectReopen `
        -Topology multiwindow `
        -RunId $runId `
        -ExpectedProjects @($multiwindowProjectA, $multiwindowProjectB) `
        -RootProcessIds @($multiwindowRestarted.Id) `
        -StartedAt $multiwindowHostRestartStartedAt
    $multiwindowGlobalAfterHostRestart = Invoke-TopologyGlobalStatus `
        -Endpoint $multiwindowEndpoint `
        -RunId $runId `
        -ProbeId 'multiwindow-after-host-restart'
    if (
        $multiwindowGlobalAfterHostRestart.processId -ne
            $multiwindowRestartedGlobal.Id
    ) {
        throw 'The multiwindow host restart changed the global owner.'
    }
    $multiwindowLogQuality = Wait-ForTopologyFailureLogQuality `
        -Topology multiwindow `
        -RunId $runId `
        -StartedAt $multiwindowStartedAt `
        -ExpectedHostStreamCount 2 `
        -ExpectedCompletionCount 4 `
        -ExpectedReopenCount 2
    $multiwindowFailureIsolation = [ordered]@{
        globalProcess = [ordered]@{
            initial = [ordered]@{
                processId = $multiwindowGlobal.Id
                status = $multiwindowGlobalStatus
                singleton = $multiwindowGlobalSingleton
                visibleWindowCount = $multiwindowGlobalWindowCount
                processes = $multiwindowGlobalMetrics
            }
            termination = $multiwindowGlobalCrash
            unavailableBeforeExplicitRestart = $multiwindowGlobalUnavailable
            noAutomaticRestartObserved = (
                $multiwindowProcessesWhileGlobalUnavailable.
                    unexpectedProcessIds.Count -eq 0
            )
            processSetWhileUnavailable = (
                $multiwindowProcessesWhileGlobalUnavailable
            )
            windowsWhileUnavailable = (
                $multiwindowWindowsWhileGlobalUnavailable
            )
            offlineContinuity = (
                Select-TopologyProbeReport -Probe $multiwindowOfflineProbe
            )
            explicitRestart = [ordered]@{
                previousProcessId = $multiwindowGlobal.Id
                processId = $multiwindowRestartedGlobal.Id
                pidChanged = (
                    $multiwindowGlobal.Id -ne $multiwindowRestartedGlobal.Id
                )
                status = $multiwindowRestartedGlobalStatus
                singleton = $multiwindowRestartedSingleton
            }
            onlineContinuity = (
                Select-TopologyProbeReport -Probe $multiwindowOnlineProbe
            )
        }
        projectHost = [ordered]@{
            termination = $multiwindowHostCrash
            forcedHostProcessId = $multiwindow.Id
            hostSurvived = $false
            otherHostSurvived = $false
            noAutomaticRestartObserved = $null -eq (
                Get-Process `
                    -Id $multiwindow.Id `
                    -ErrorAction SilentlyContinue
            )
            processSetAfterCrash = $multiwindowProcessesAfterHostCrash
            remainingWindowCount = $multiwindowStoppedWindows.observedCount
            survivorContinuity = $null
            explicitRestart = [ordered]@{
                previousProcessId = $multiwindow.Id
                processId = $multiwindowRestarted.Id
                pidChanged = (
                    $multiwindow.Id -ne $multiwindowRestarted.Id
                )
                ready = $multiwindowRestartedWindows
                cacheReady = $multiwindowRestartedCache
                reopen = $multiwindowReopen
                globalStatus = $multiwindowGlobalAfterHostRestart
            }
        }
        ipc = New-TopologyIpcEvidence `
            -Topology multiwindow `
            -SuccessfulProjectProbeCount 4
        logs = $multiwindowLogQuality
    }
    Stop-OwnedTopologyProcess -ProcessId $multiwindowRestarted.Id
    Stop-OwnedTopologyProcess -ProcessId $multiwindowRestartedGlobal.Id
    }

    $topologyRunners = @{
        independent = $runIndependentTopology
        multiwindow = $runMultiwindowTopology
    }
    $topologyRunOrder = if ($ExecutionOrder -eq 'AB') {
        @('independent', 'multiwindow')
    }
    else {
        @('multiwindow', 'independent')
    }
    for ($topologyIndex = 0; $topologyIndex -lt 2; $topologyIndex++) {
        if ($topologyIndex -gt 0) {
            Start-Sleep -Milliseconds 750
        }
        $topologyRunner = $topologyRunners[$topologyRunOrder[$topologyIndex]]
        . $topologyRunner
    }
    Assert-ComparableCanvasTargets `
        -Independent $independentInteraction `
        -Multiwindow $multiwindowInteraction

    & (Join-Path $PSScriptRoot 'Prepare-TopologyCorpus.ps1')
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    $corpusManifestAfterRuns = Get-Content `
        -LiteralPath $corpusManifestPath `
        -Raw `
        -Encoding utf8 |
            ConvertFrom-Json
    if (
        $corpusManifestAfterRuns.corpusSha256 -ne $corpusManifest.corpusSha256 -or
        $corpusManifestAfterRuns.totalFiles -ne $corpusManifest.totalFiles -or
        $corpusManifestAfterRuns.totalBytes -ne $corpusManifest.totalBytes
    ) {
        throw 'The real-image corpus changed while the topology spike was running.'
    }

    $currentInputState = Get-BuildInputState
    $corpusPhotoCount = [long](
        (
            $corpusManifest.albums |
                ForEach-Object { $_.photos.Count } |
            Measure-Object -Sum
        ).Sum
    )
    $independentOfflineComplete = (
        $independentFailureIsolation.globalProcess.offlineContinuity.
            missingCompletions -eq 0
    )
    $independentReopenComplete = (
        $independentFailureIsolation.projectHost.explicitRestart.
            reopen.observedProjects -eq 1
    )
    $multiwindowOfflineComplete = (
        $multiwindowFailureIsolation.globalProcess.offlineContinuity.
            missingCompletions -eq 0
    )
    $multiwindowReopenComplete = (
        $multiwindowFailureIsolation.projectHost.explicitRestart.
            reopen.observedProjects -eq 2
    )
    $failureGatePassed = (
        $independentOfflineComplete -and
        $independentReopenComplete -and
        $multiwindowOfflineComplete -and
        $multiwindowReopenComplete -and
        -not $buildManifest.buildInputsDirty -and
        $currentInputState.digestSha256 -eq
            $buildManifest.buildInputDigestSha256 -and
        $imagingRecovery.validated
    )
    $report = [ordered]@{
        schemaVersion = 12
        collectedAtUtc = [DateTime]::UtcNow.ToString('o')
        execution = [ordered]@{
            order = $ExecutionOrder
        }
        hardware = Get-HardwareInventory
        corpus = [ordered]@{
            schemaVersion = $corpusManifest.schemaVersion
            albumCount = $corpusManifest.albums.Count
            mediaCount = $corpusManifest.totalFiles
            photoCount = $corpusPhotoCount
            decorativeCount = 1
            sourceBytes = $corpusManifest.totalBytes
            corpusSha256 = $corpusManifest.corpusSha256
            integrity = [ordered]@{
                verified = $true
                beforeSha256 = $corpusManifest.corpusSha256
                afterSha256 = $corpusManifestAfterRuns.corpusSha256
            }
            previewPolicy = [ordered]@{
                representationsPerMedia = 1
                opaqueFormat = 'jpeg'
                transparentFormat = 'png'
                maximumEdgePx = 1600
            }
        }
        build = [ordered]@{
            manifestVersion = $buildManifest.manifestVersion
            builtAtUtc = $buildManifest.builtAtUtc
            gitCommit = $buildManifest.gitCommit
            workingTreeDirty = $buildManifest.workingTreeDirty
            buildInputsDirty = $buildManifest.buildInputsDirty
            buildInputFileCount = $buildManifest.buildInputFileCount
            buildInputDigestSha256 = $buildManifest.buildInputDigestSha256
            executable = $buildManifest.executable
            executableSha256 = $buildManifest.executableSha256
            imagingExecutable = $buildManifest.imagingExecutable
            imagingExecutableSha256 = $buildManifest.imagingExecutableSha256
            profile = $buildManifest.profile
            currentBuildInputsMatchManifest = (
                $currentInputState.digestSha256 -eq
                    $buildManifest.buildInputDigestSha256
            )
        }
        alternatives = [ordered]@{
            independentHosts = [ordered]@{
                ready = $independentReady
                cache = $independentCache
                interaction = $independentInteraction
                processes = $independentMetrics
                forcedFailure = $independentFailureIsolation
            }
            multiwindowHost = [ordered]@{
                ready = $multiwindowReady
                cache = $multiwindowCache
                interaction = $multiwindowInteraction
                processes = $multiwindowMetrics
                forcedFailure = $multiwindowFailureIsolation
            }
        }
        failureGate = [ordered]@{
            passed = $failureGatePassed
            imagingProcessor = $imagingRecovery
        }
        notMeasured = @($reportText.notMeasured)
        notes = @($reportText.notes)
    }

    $outputDirectory = Split-Path -Parent $OutputPath
    New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
    $json = $report | ConvertTo-Json -Depth 16
    [System.IO.File]::WriteAllText(
        $OutputPath,
        $json + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    $summaryPath = [System.IO.Path]::ChangeExtension($OutputPath, '.md')
    Write-TopologyMarkdownSummary `
        -Report $report `
        -Text $reportText `
        -SummaryPath $summaryPath
    Write-Output "Topology spike report: $OutputPath"
    Write-Output "Topology spike summary: $summaryPath"
    Write-Output $json
}
finally {
    foreach ($processId in @($startedProcessIds)) {
        try {
            Stop-OwnedTopologyProcess -ProcessId $processId
        }
        catch {
            Write-Warning $_.Exception.Message
        }
    }
    $gateRoot = [System.IO.Path]::GetFullPath($probeGateDirectory) +
        [System.IO.Path]::DirectorySeparatorChar
    foreach ($gatePath in $probeGatePaths) {
        $fullGatePath = [System.IO.Path]::GetFullPath($gatePath)
        if (
            $fullGatePath.StartsWith(
                $gateRoot,
                [System.StringComparison]::OrdinalIgnoreCase
            ) -and
            (Test-Path -LiteralPath $fullGatePath -PathType Leaf)
        ) {
            Remove-Item -LiteralPath $fullGatePath -Force
        }
    }
    if (-not [string]::IsNullOrWhiteSpace($failureScratchRoot)) {
        try {
            Remove-TopologyFailureScratchRoot -Path $failureScratchRoot
        }
        catch {
            Write-Warning $_.Exception.Message
        }
    }
    Set-ProcessEnvironmentValue -Name 'CARGO_TARGET_DIR' -Value $previousCargoTarget
}
