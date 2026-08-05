---
status: historical
document: technical-research
ticket: fase-2-fluxo-persistente-04
date: 2026-08-03
updated: 2026-08-05
---

# Provar o Salvamento atômico preservando a trava do Projeto

Type: prototype

Status: resolved

Blocked by: 01 — Reconciliar a fronteira de caminhos da próxima fase

## Question

Qual sequência concreta de temporário irmão, gravação, flush, substituição, verificação e transferência ou reaquisição de `ProjectFileLock` preserva a revisão confirmada e evita um intervalo desprotegido durante o Salvamento?

O protótipo deve cobrir sucesso, conflito de revisão, falha, interrupção e as diferenças relevantes entre destino local e UNC, sem decidir pelo pathname nem prometer atomicidade que o sistema de arquivos não oferece.

## Prototype

- Branch: `codex/prototype-atomic-project-save`
- Commit: `95df85f` (`prototype: prove locked atomic project save`)
- Worktree: `C:\Users\Usuario\AppData\Local\Temp\my-albuns-prototype-atomic-save`
- Run: `npm run prototype:atomic-save`
- Validação automatizada: oito cenários aprovados por acesso local e por SMB loopback; `cargo clippy -D warnings`, testes de `myalbuns-paths`, formatação e revisão independente sem achados materiais restantes.
- Validação humana: aceita. A reação ao protótipo aprovou a barreira irmã derivada da Identidade e a política de encerrar a Sessão fail-closed quando o resultado físico for inconclusivo.

## Answer

A transferência direta entre duas `ProjectFileLock` não é viável: manter os handles travados do objeto antigo e do temporário faz o próprio `ReplaceFileW` falhar com violação de compartilhamento, tanto localmente quanto pelo SMB loopback.

O `ProjectStore` deverá coordenar cada abertura e Salvamento por uma barreira estável irmã, derivada de uma chave opaca da Identidade persistida. Sob essa barreira, o Salvamento grava e sincroniza o temporário irmão, revalida por handle o alvo e a candidata, libera a trava antiga, executa `ReplaceFileW` com flags `0`, adquire a nova `ProjectFileLock` e compara novamente identidade física, pathname e bytes exatos do payload antes de liberar a barreira e chamar `ProjectSession::confirm_saved_revision`.

Um erro do replace deve ser classificado pelo objeto físico que ocupa o destino. A trava correspondente só substitui a barreira após comparação conclusiva. Se o resultado permanecer inconclusivo, a Sessão editável é encerrada antes da liberação dos guards; não se repete o Salvamento nem se confirma a revisão. O host serializa comandos da Sessão durante esse I/O; permitir edição concorrente exigiria futuramente outro contrato de snapshot/token.

Interrupções reais antes e depois do replace deixaram, respectivamente, a revisão anterior ou a candidata completa e permitiram reabertura depois da morte do owner. Os resultados local e SMB loopback comprovam essa sequência neste ambiente, mas não prometem atomicidade ou durabilidade universal de todo filesystem ou servidor UNC.
