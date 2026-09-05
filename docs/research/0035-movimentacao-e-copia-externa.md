---
status: current
document: technical-research
ticket: 10-movimentacao-copia-externa
date: 2026-08-17
updated: 2026-08-30
---

# Movimentação e Cópia externa

## Pergunta

Como provar, pelas fronteiras públicas do `ProjectCore` e por processos e
caminhos reais do Windows, que movimentar um Projeto conserva sua Identidade,
enquanto copiar seu arquivo produz uma Identidade própria antes de qualquer
Sessão ou namespace local?

## Contrato exercitado

O gate de operação da issue 10 cobre três resultados fechados da comparação de
identidade:

- `Same` reutiliza a Sessão proprietária e fornece a Identidade exata da
  instância proprietária, composta por PID e `FILETIME` de criação, para
  focalização;
- `Different` promove a Cópia externa gravável por uma escrita técnica que muda
  apenas a Identidade persistida;
- evidência inconclusiva, inclusive uma raiz anterior indisponível, não é
  convertida em movimentação nem em Cópia externa.

Uma movimentação só é inferida quando a localização anterior está
comprovadamente ausente. Nesse caso, o registro durável recebe a nova
localização, mas o UUID e o namespace derivado permanecem iguais. Se o mesmo
objeto físico estiver ativo por outro alias, o lease de Identidade contém a
Identidade exata da instância do Host proprietário e o Host efêmero devolve
`FocusExisting` sem criar uma segunda Sessão. O Global abre e revalida essa
instância e conserva seu handle durante toda a enumeração e tentativa de foco;
o PID isolado permanece apenas diagnóstico.

Uma Cópia externa gravável recebe um UUID novo pelo protocolo de substituição
atômica do `ProjectStore`. O candidato técnico é derivado dos bytes persistidos
da fonte; por isso, uma alteração criativa ainda pendente na Sessão do original
não participa dessa escrita. O lease da nova Identidade é publicado antes de o
`EditableProject` ser devolvido ao Host, que por sua vez só cria WebView, Cache
e Recuperação depois do bootstrap concluído.

Quando a correção no próprio arquivo encontra `AccessDenied`, o Core devolve
uma fonte opaca e nenhuma Sessão. `save_copy_as(SaveCopyAsRequest)` revalida a
fonte por handle, publica um documento no schema atual com UUID novo e a mesma
Revisão, preserva a fonte byte a byte e só então devolve a Sessão editável. O
Host que recusou a abertura permanece vivo com essa mesma fonte opaca enquanto
o Global abre o diálogo; somente a autoridade do Destino retorna ao processo.
Assim, a segunda etapa não reabre nem tenta corrigir a fonte.

A barreira curta de transição não cria mais um arquivo irmão no volume do
Projeto. Ela é derivada da Identidade dentro do `identity_lease_root` local e
autorizado do Core, onde continua serializando processos da mesma instalação.
Isso permite abrir e validar uma fonte em mídia realmente protegida contra
escrita; só a tentativa técnica de substituir sua Identidade toca o volume e
produz o terminal acionável `ExternalCopyNotWritable`.

## Instrumento

`scripts/Test-Issue10IdentityGate.ps1` compila o exemplo público
`issue10_identity_gate` e fornece todos os caminhos e modos por argumentos
explícitos. O cenário não seleciona comportamento por variável de ambiente,
`cfg(test)` produtivo ou hook interno.

O runner cria um compartilhamento SMB loopback pelo compartilhamento
administrativo do volume e mapeia uma letra livre. Um processo real mantém o
Projeto original aberto com DPI 240 ainda não salvo. Sua linha de prontidão em
stdout é a relação causal para as tentativas seguintes; a liberação chega por
stdin, sem `sleep` como sincronização. Outros processos então:

1. abrem o mesmo arquivo pela unidade mapeada e pelo alias UNC;
2. promovem uma cópia física enquanto o original continua aberto;
3. persistem DPI 600 somente na cópia e confirmam DPI 300 no original;
4. renomeiam e movem o original fechado, conservando UUID e namespace;
5. abrem uma cópia somente leitura de um documento schema 1;
6. salvam a cópia no schema 2, mesma Revisão e novo namespace;
7. recusam um Destino ocupado sem alterar fonte ou Destino;
8. desmontam a unidade que continha uma localização anterior e confirmam
   `IdentityIndeterminate` sem reescrever o candidato.

Antes da jornada SMB, o mesmo runner executa ainda duas provas causais pelo
seam público do `ProjectCore`. A primeira usa um oplock real para interromper a
leitura de A, troca o pathname por uma cópia física B com o mesmo UUID e só
então libera a abertura; o resultado obrigatório é `IdentityIndeterminate`.
A segunda cria uma imagem ISO por IMAPI2, monta-a como unidade óptica pelo
Windows e exige `ExternalCopyNotWritable` ao abrir a Cópia externa nela. Ambas
usam eventos ou stdin para coordenação, sem `sleep`.

O runner também fixa a comparação entre os dois domínios de identidade física
do Windows: somente IDs do mesmo formato podem produzir `Same`; uma diferença
de ID só produz `Different` no mesmo formato, enquanto formatos mistos no mesmo
volume produzem `Indeterminate`. Como `FILE_ID_INFO` fornece um serial de volume
de 64 bits, enquanto `BY_HANDLE_FILE_INFORMATION` fornece 32 bits, igualdade ou
desigualdade crua entre esses campos nunca decide a comparação. Em um handle
local, uma segunda observação por `GetVolumeInformationByHandleW` fornece o
serial comparável de 32 bits: sua divergência pode provar volumes distintos e
`Different`, mas sua igualdade não torna formatos de ID mistos comparáveis. Em
SMB, onde essa API de volume não é suportada, o domínio misto permanece
`Indeterminate`.

IDs estendidos compostos somente por `00` ou somente por `FF` são
sentinelas sem autoridade e são ignorados. Um ID legado só é materializado com
proveniência tipada de filesystem e garantia documental; igualdade legada em
ReFS ou filesystem desconhecido, formato legado antigo sem proveniência e erro
inesperado na consulta estendida terminam em `Indeterminate`.

O fallback não é uma segunda tentativa incondicional. O Core captura
`GetLastError` imediatamente após `GetFileInformationByHandleEx(FileIdInfo)` e
consulta o tipo do filesystem pelo mesmo handle. `ERROR_INVALID_FUNCTION` e
`ERROR_NOT_SUPPORTED` indicam ausência real da classe estendida; o
`ERROR_INVALID_PARAMETER` observado em mídia UDF só é aceito para UDF, cujo ID
de 64 bits é garantido. NTFS e UDF podem então produzir um ID legado tipado;
ReFS, CDFS, filesystem desconhecido e qualquer erro inesperado permanecem sem
evidência autoritativa. Em SMB, onde a consulta do nome do filesystem por
handle não é suportada, somente um `FILE_ID_INFO` estendido válido autoriza a
comparação. Isso conserva os aliases UNC positivos e fecha qualquer downgrade
de evidência em falha transitória. O token local estendido v2 transporta
separadamente ambos os seriais quando a observação comparável existe; o token
v1 anterior continua aceito sem inventar essa prova e, por isso, permanece
inconclusivo contra um ID legado.

O artefato canônico é
[`artifacts/0035-issue-10-identity-gate.json`](artifacts/0035-issue-10-identity-gate.json).
Ele é a única fonte dos UUIDs, da Identidade de instância PID+`FILETIME`, dos
hashes, do commit e do sistema operacional da rodada registrada.
`sourceInputsDirty=false` exige o mesmo `HEAD` e uma árvore inteira limpa antes
e depois da jornada, excluindo apenas o próprio JSON de saída.

## Cobertura complementar

Os testes Rust de integração atravessam `ProjectCore` para movimento, alias
físico, promoção automática, escrita técnica sem estado criativo pendente,
fonte que volta a ser gravável, `Salvar cópia como...`, Destino ocupado,
cancelamento por descarte da fonte opaca, localização anterior reutilizada,
troca A→B durante uma abertura bem-sucedida e volume ISO protegido contra
escrita. A identidade física usa primeiro o ID de arquivo de 128 bits e mantém
como fallback tipado o par `VolumeSerialNumber + FileIndex` somente quando a
consulta estendida falha como “não suportada” e o filesystem possui garantia
aplicável; os formatos nunca compartilham o mesmo token local nem são
comparados como se fossem o mesmo domínio de identidade. Testes da primitiva
cobrem ambas as sentinelas de 128 bits, ReFS e filesystem desconhecido, erro
inesperado sem fallback, NTFS/UDF positivos e propagação pela lease até impedir
`FocusExisting`. Uma regressão usa os valores nativos de um mesmo volume NTFS
(`a8f2cdd3f2cda5c2` estendido e `f2cda5c2` legado) para provar que a diferença
de largura não autoriza `Different` nem promoção; outra consulta os dois
formatos no mesmo handle NTFS real usado pelo runner.
Os testes de Host e protocolo comprovam a correlação de `FocusExisting`, a
eliminação do Host efêmero e o transporte separado das autoridades de fonte e
Destino. A decisão acionável não atravessa mais como terminal público para o
frontend da Tela Global: a mesma janela externa que apresentava o progresso de
abertura transiciona para **Salvar cópia como…** ou **Cancelar**, enquanto o
Host, a fonte opaca e a tentativa permanecem correlacionados. O seletor nativo
de Destino pertence a essa janela. Cancelar somente esse seletor volta à mesma
decisão e conserva o Host; **Cancelar** na decisão descarta e recolhe o Host
pendente, exigindo nova abertura para uma nova tentativa.

## Limites

- o SMB loopback exercita aliases e indisponibilidade reais, mas não representa
  latência WAN, DFS ou troca de credenciais;
- o gate observa a Identidade exata da instância retornada pelo Core e a
  ausência do Host duplicado; o Global retém o handle revalidado durante a
  focalização, mas a permissão do Windows para trazer uma janela de outro
  processo ao primeiro plano ainda depende das regras normais de foreground do
  sistema;
- o Core não monta Cache ou WebView. A ordem “Identidade antes de namespace” é
  completada pelos contratos públicos do Host, enquanto o gate confirma que o
  namespace derivado já é distinto no primeiro resultado editável;
- os terminais da janela externa e o cancelamento do seletor nativo são cobertos
  na fronteira Global e no teste de aplicação; a jornada SMB não automatiza um
  diálogo modal do Windows.
- a prova de mídia somente leitura depende dos serviços nativos IMAPI2 e Disco
  Virtual disponíveis no Windows; ela valida a recusa de escrita antes de
  expor a unidade ao teste.

## Fontes Win32 e SMB

- [MS-FSCC 2.1.10 — FileId](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fscc/98860416-1caf-4c80-a9ab-8d61e1ccf5a5):
  os valores de 128 bits todo zero e todo `FF` devem ser ignorados;
- [MS-FSCC 6 — Product Behavior](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fscc/d4bc551b-7aaf-4b4f-ba0e-3a75e7c528f0):
  matriz de garantias dos IDs de 64 e 128 bits por NTFS, ReFS, UDFS e CDFS;
- [`BY_HANDLE_FILE_INFORMATION`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/ns-fileapi-by_handle_file_information):
  o índice de 64 bits não é garantidamente único em ReFS;
- [`FILE_ID_INFO`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info):
  o serial de volume do domínio estendido é um `ULONGLONG`;
- [`GetFileInformationByHandleEx`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex):
  contrato de `FileIdInfo` e suporte SMB;
- [`GetVolumeInformationByHandleW`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumeinformationbyhandlew):
  identificação do filesystem pelo handle e ausência de suporte SMB;
- [The Old New Thing — IDs sentinela](https://devblogs.microsoft.com/oldnewthing/20220127-00/?p=106199)
  e [ordem segura de fallback](https://devblogs.microsoft.com/oldnewthing/20220128-00/?p=106201):
  exemplos oficiais da Microsoft para descartar sentinelas, tentar primeiro o
  ID de 128 bits e só então considerar um ID legado suportado.

## Repetição

Em Windows, com o compartilhamento administrativo local disponível:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-Issue10IdentityGate.ps1 `
  -OutputPath docs\research\artifacts\0035-issue-10-identity-gate.json
```

O runner só substitui o artefato depois que todas as relações observadas
passam e sempre desmonta a unidade e remove o scratch da rodada.
