---
status: historical
document: technical-research
ticket: fase-2-fluxo-persistente-05
date: 2026-08-03
updated: 2026-08-05
---

# Materializar o bootstrap entre Boas-vindas e Host do Projeto

Type: prototype

Status: resolved

Blocked by: 01 — Reconciliar a fronteira de caminhos da próxima fase

## Question

Qual é o menor protocolo de bootstrap que permite ao `MyAlbuns.exe` global criar ou selecionar um arquivo, iniciar exatamente um host independente por Projeto, entregar o alvo com autoridade estreita e receber estado pronto ou falha, sem hospedar `ProjectSession` nem ressuscitar o processo global candidato removido ao fim da Fase 1?

O protótipo deve tratar cancelamento do diálogo, falha de inicialização, correlação da tentativa e encerramento de responsabilidade do processo global; nomes finais e um protocolo IPC genérico continuam fora do objetivo.

## Prototype

- Branch: `codex/prototype-project-host-bootstrap`
- Commit: `78a9fa1015f958419bf8547ae255e2fbf29d4657`
- Worktree: `C:\Users\Usuario\AppData\Local\Temp\my-albuns-prototype-host-bootstrap`
- Run: `npm run prototype:host-bootstrap`
- Evidência automatizada: os 11 cenários passaram; `cargo fmt --all -- --check`, `cargo check`, `cargo clippy -- -D warnings` e os testes da biblioteca passaram (67 passaram, 3 integrações específicas ignoradas como previsto).
- Revisão humana: aprovada em 2026-08-03 após a avaliação dos cenários 1–11.

## Answer

Aprovado pelo responsável do produto em 2026-08-03 após a validação do protótipo.

O bootstrap mínimo adotado é um protocolo descartável de uma requisição JSON pelo `stdin` herdado e um único terminal JSON pelo `stdout`. O processo global abre o diálogo e inicia o Host, mas não abre nem cria a `ProjectSession`: entrega um `RootBindingPlan` congelado cujo caminho permanece no DTO nativo `windowsUtf16`, e o Host valida o alvo, cria ou abre o Projeto e passa a possuir exatamente uma sessão editável.

Cada tentativa carrega identificador, nonce e PID. O processo global só libera o Host após receber `Ready` correlacionado dentro do prazo; terminal inválido, correlação divergente, falha ou timeout encerram o Host de forma fail-closed. Depois de `Ready`, o processo global pode terminar sem encerrar o Host. A trava por Identidade limita cada alvo a um Host pronto, enquanto alvos diferentes continuam independentes.

Para `CreateNew`, o protótipo materializou a autorização `CreateOnly` e comprovou seus terminais de bootstrap. O contrato final em [Contrato público de persistência do ProjectCore](../../../design/0015-contrato-publico-de-persistencia-do-project-core.md) aperta a ordem interna: o Host grava e sincroniza o temporário, publica, verifica e bloqueia o arquivo final e só então abre a única Sessão. Ele também acrescenta `ReplaceConfirmed` como campo irmão do `RootBindingPlan` no envelope `CreateNew`, congelado quando o Windows tiver avisado o conflito e o usuário autorizado a substituição; o plano continua contendo somente bindings de caminhos. Essa extensão não altera o protocolo descartável de bootstrap; sua sequência de substituição e verificação deriva da prova de Salvamento atômico, não da evidência `CreateOnly` deste protótipo. Falhas tratadas liberam os recursos observados; uma queda abrupta ainda pode deixar o temporário privado ou o Projeto final já completo, nunca um arquivo final parcial. O protótipo não fixa nomes finais de executáveis, uma camada IPC genérica nem um coordenador adicional.

Os 11 cenários do protótipo foram aprovados, incluindo cancelamento, falhas antes e depois da sessão, timeout, correlação incorreta, concorrência por alvo e sobrevivência do Host após a saída do processo global.
