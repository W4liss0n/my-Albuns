---
status: current
document: technical-research
ticket: 10-movimentacao-copia-externa
date: 2026-08-17
updated: 2026-08-18
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
como fallback tipado o par `VolumeSerialNumber + FileIndex` documentado pelo
Windows para filesystems que não oferecem `FileIdInfo`; os formatos nunca
compartilham o mesmo token local.
Os testes de Host e protocolo comprovam a correlação de `FocusExisting`, a
eliminação do Host efêmero e o transporte separado das autoridades de fonte e
Destino. Os testes da Tela Global verificam que o frontend não recebe pathname,
oferece **Salvar cópia como…** apenas no resultado acionável e mantém a oferta
quando uma abertura posterior é cancelada ou falha antes de substituir o Host
pendente. Cancelar o diálogo de **Salvar cópia como…** descarta e reap o Host
pendente; a fonte precisa ser aberta novamente para uma nova tentativa.

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
- o cancelamento do diálogo nativo é coberto na fronteira Global e no teste de
  aplicação; a jornada SMB não automatiza um diálogo modal do Windows.
- a prova de mídia somente leitura depende dos serviços nativos IMAPI2 e Disco
  Virtual disponíveis no Windows; ela valida a recusa de escrita antes de
  expor a unidade ao teste.

## Repetição

Em Windows, com o compartilhamento administrativo local disponível:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File scripts\Test-Issue10IdentityGate.ps1 `
  -OutputPath docs\research\artifacts\0035-issue-10-identity-gate.json
```

O runner só substitui o artefato depois que todas as relações observadas
passam e sempre desmonta a unidade e remove o scratch da rodada.
