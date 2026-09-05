---
status: current
document: research
date: 2026-09-04
---

# Isolamento do fechamento de Projeto salvo

O travamento nativo de **Arquivo → Fechar Projeto** continua sem causa
confirmada. Esta rodada acrescenta provas sem janelas e observabilidade; não
constitui uma correção do defeito nem aprovação da jornada produtiva da PR #64.

## Evidência de partida

A reprodução nativa local de `a0d6c6fa941dbd141026629633d159a233cb5e69`
foi preservada em
`.scratch/productive-journey-evidence/pr64-close-1/failure.json` e
`.tools/logs/pr64-original-close-1.log` (artefatos locais fora do Git).

A sequência altera o original, usa Salvar como para adotar uma cópia, reabre o
original em outro Host, altera/salva ambos e fecha o original. Após o clique,
os menus do original ficam desabilitados e a janela permanece aberta. O último
salvamento de cada Host está registrado, mas não aparece
`clean_project_close_requested`.

Esse evento era emitido **depois** de `ProjectHost::begin_close()`. Sua ausência
não permite concluir que o IPC não chegou ao Rust: a aquisição da sessão, a
limpeza de Recuperação ou a liberação da sessão ainda poderiam estar pendentes.
O evento de salvamento no Rust também não prova que a Promise da interface
recebeu a resposta.

## Provas sem janelas

Foram executados inicialmente 15 testes de fechamento da interface e 27 testes
de sessão do Host, todos aprovados. Foram acrescentadas estas três provas:

- Duas variantes em `src/components/ProjectWorkspace.test.tsx`: aplicar uma
  alteração, salvar e solicitar fechamento com o salvamento ainda pendente ou
  já concluído. Exercitam o menu, a atualização da projeção, a fila real de
  mutações, o controlador e o adaptador de janela Tauri. As operações do Core e
  a chamada final `invoke` são simuladas. Ambas enviam `request_project_close`
  exatamente uma vez, após a resposta do salvamento.
- `saved_original_closes_after_save_as_and_independent_copy_edits`, em
  `src-tauri/src/project_host.rs`: usa Core, arquivos, bloqueios e Recuperação
  reais, faz Salvar como, reabre o original, altera/salva original e cópia em
  workers e fecha o original com limite de cinco segundos. Confirma a liberação
  do original para reabertura e a preservação da cópia. Passou em 0,28 s na
  primeira execução. As duas sessões residem no mesmo processo de teste; não há
  IPC, WebView2 ou transição de autoridade do WebView.

Comandos focados, executados a partir da raiz do repositório:

```powershell
npm test -- src/components/ProjectWorkspace.test.tsx -t 'dispatches close to IPC'
```

```powershell
. ./scripts/Local-Toolchain.ps1
Initialize-MyAlbunsToolchain
& $script:CargoExecutable test -p myalbuns-desktop --lib saved_original_closes_after_save_as_and_independent_copy_edits
```

O teste da interface levou 3,58 s incluindo preparação, com 0,57 s nos dois
casos. A primeira compilação Rust levou 44,84 s. Os logs desta rodada ficam em
`.tools/logs/close-isolation-*.log`.

Esses resultados reduzem as hipóteses, mas nenhuma prova nova reproduziu o
travamento observado. Não se deve descrevê-las como uma regressão vermelha que
ficou verde, nem substituir a integração nativa por seus resultados.

## Validação adicional e estabilidade das provas

A suíte dos três arquivos de interface afetados passou: 147 testes, além do
TypeScript. Ao ampliar para os testes Rust de Projeto, 118 passaram, três
falharam e um permaneceu ignorado. As falhas ocorreram em testes anteriores que
presumiam publicação em 90–100 ms:

- `save_reports_failed_recovery_cleanup_until_a_later_save_finishes_it`;
- `a_checkpoint_from_an_older_saved_base_is_never_discarded_implicitly`;
- `failed_finish_preserves_the_pending_publication`.

Os dois primeiros passaram a observar o checkpoint com o auxiliar existente,
limitado a cinco segundos. O terceiro agenda a publicação com prazo distante e
executa diretamente o mesmo worker de publicação na geração capturada, somente
após remover a obstrução. Assim, verifica a preservação do trabalho pendente
após a falha, sem disputar o timer com o preparo da fixture. O teste de debounce
permanece separado. Nenhuma regra produtiva de Recuperação foi alterada.

Após esses ajustes, a mesma seleção Rust `project_` passou: 121 testes,
nenhuma falha e um teste de integração do processador já ignorado. Clippy
(`-p myalbuns-desktop --lib --tests -- -D warnings`) e `git diff --check`
também passaram. A revisão estática de padrões e especificação não identificou
achados no isolamento e na observabilidade; a confirmação nativa segue pendente.

## Observabilidade acrescentada

O controlador registra a tentativa por `operationId`, pelo logger já existente:

| Evento | Ponto observado |
| --- | --- |
| `project_close_requested` | O comando foi aceito e iniciou a espera pela fila. |
| `project_close_mutations_settled` | A espera terminou; `reason` distingue fila vazia, sucesso, falha ou operação obsoleta. |
| `project_close_ipc_requested` | O controlador está prestes a chamar a porta de janela. |
| `project_close_ipc_resolved` | A porta retornou fechamento ou necessidade de confirmação. |
| `project_close_request_failed` | A espera ou a porta rejeitou a operação. |

O Host registra, com PID, `project_close_command_received` antes de tentar
adquirir a sessão, `project_close_session_acquired` após adquirir o mutex e
`project_close_recovery_finishing` / `project_close_recovery_finished` ao redor
da limpeza de Recuperação de uma sessão limpa. O evento já existente
`clean_project_close_requested` continua depois do retorno de `begin_close`,
quando os recursos da sessão foram liberados.

Os registros não mudam a decisão, não inserem timeout na operação produtiva e
não incluem caminhos de arquivo ou conteúdo do Projeto. Uma exceção do logger
da interface não pode impedir o fechamento. Os eventos da interface seguem pelo
IPC existente; se esse canal inteiro travar, sua ausência nos logs do Host
continua inconclusiva. A ordem entre registros de canais diferentes também não
é uma prova de ordem de execução. `operationId` correlaciona os registros da
interface; PID e arquivo de log identificam o Host, sem alterar o contrato IPC.

## Hipóteses ainda não comprovadas e próxima evidência

1. Uma resposta de salvamento não retorna à interface na execução nativa. A
   previsão é observar `project_close_requested` sem
   `project_close_mutations_settled`, apesar de `project_save_completed` no Host.
2. O comando chega ao Host e fica bloqueado. O último evento de aquisição ou
   limpeza deve localizar essa espera; uma pilha do processo suspenso pode
   identificar quem possui o recurso.
3. A falha depende do transporte ou ciclo de vida nativo. A previsão é que a
   interface ultrapasse a fila enquanto o comando não entra no Host, ou que a
   sessão seja consumida sem a janela terminar. Os testes isolados não
   distinguem essas possibilidades.

A evidência necessária agora é uma execução curta do mesmo caso em Windows
isolado com WebGL2 por hardware, retendo as etapas acima e, se houver bloqueio,
console/estado da interface e pilha do Host antes de encerrar o processo. A
captura deve distinguir a janela do original da cópia e registrar o commit e
os executáveis usados.

Nenhuma janela nativa foi aberta nesta rodada. O runner comum `windows-2022`
já falhou na inicialização gráfica e não fornece essa prova. Permanecem as
restrições e os requisitos de autorização de
`docs/agents/native-ui-gates.md`. Não repetir a jornada inteira nem alterar o
fechamento com base em uma hipótese ainda não reproduzida.
