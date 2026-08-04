# 02 — Abrir um Projeto v1 em um Host independente

**What to build:** permitir que a pessoa abra um `.myalbuns` v1 pela Tela de Boas-vindas e chegue ao editor hospedado por um processo próprio, sem o processo global possuir estado criativo.

**Blocked by:** 01 — Expandir o ProjectCore com o Documento v1.

**Type:** implementation

**Status:** resolved

**Normative sources:** [ticket pai](../../programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md); [Tela de Boas-vindas](../../../docs/design/0002-tela-de-boas-vindas.md); [política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md); [contrato público de persistência](../../../docs/design/0015-contrato-publico-de-persistencia-do-project-core.md); [bootstrap aprovado](../../fase-2-fluxo-persistente/issues/05-materializar-o-bootstrap-do-host-de-projeto.md).

- [x] Iniciar o aplicativo sem arquivo mostra a Tela de Boas-vindas, sem Canvas, `ProjectSession`, Projeto demonstrativo ou Processador pertencente ao processo global.
- [x] `Abrir Projeto` usa o diálogo nativo do Windows e transporta o pathname confirmado como DTO reversível, nunca como string Unicode do frontend; cancelar termina sem chamar o núcleo, iniciar Host ou alterar Projetos recentes.
- [x] Abrir diretamente um `.myalbuns` pelo Windows chega ao mesmo bootstrap e pode mostrar o Host sem exigir a Tela de Boas-vindas como etapa intermediária.
- [x] A tentativa congela o pathname em um `RootBindingPlan` imutável; nenhum consumidor redescobre alias ou raiz por conta própria.
- [x] A abertura atravessa somente `ProjectCore.open_editable`, executa leitura estrita e validação fora da thread da interface e cria exatamente um `EditableProject` no Host do Projeto.
- [x] O bootstrap usa uma requisição correlacionada e um único terminal estruturado; timeout, terminal inválido, correlação divergente ou falha encerram o processo iniciado de forma fechada e exibem erro acionável.
- [x] Somente `Ready` válido libera o Host e permite mostrar o editor; depois disso o Host continua vivo independentemente da Tela de Boas-vindas ou da saída do processo global.
- [x] Tentar abrir um Projeto protegido por outra Sessão retorna `ProjectInUse` sem quebrar o bloqueio existente nem criar uma segunda Sessão editável.
- [x] Uma abertura bem-sucedida promove o Projeto para o topo de Projetos recentes somente depois de `Ready`; falha anterior não cria nem reordena a entrada.
- [x] A fronteira cobre caminhos locais, não ASCII, longos, UNC, mapeados e verbatim, além de alvo inválido, acesso negado e origem indisponível, sem resolver rede na thread da interface.
- [x] Um teste de processo real comprova PIDs distintos, uma Sessão no Host, nenhuma Sessão no processo global, terminal correlacionado e continuidade do Host depois que seu pai global deixa de existir.
- [x] O corte não introduz protocolo IPC genérico, coordenador global de Sessões, Recuperação, foco por alias ou experiência completa de Cópia externa.

## Comments

- 2026-08-03 — A Tela de Boas-vindas e o Host de Projeto passaram a ter entrypoints, capabilities e ciclos de vida próprios. O processo global conserva somente diálogo, lista de recentes e bootstrap; cada Host mantém exatamente um `EditableProject` produtivo.
- O pathname confirmado permanece no backend como DTO nativo e atravessa uma requisição limitada, correlacionada e de uso único com seu `RootBindingPlan` congelado. `Ready` e `Failed` são os únicos terminais, e correlação inválida, timeout ou falha encerram e reaproveitam apenas o processo iniciado.
- O gate de processo real validou PIDs distintos, terminal correlacionado, exclusividade por `ProjectInUse`, liberação da trava ao encerrar o Host e continuidade do Host após a saída do Global. O modo headless usado por esse gate existe somente em builds de depuração porque a Área Restrita do Windows não permite criar WebView2; o caminho produtivo continua exigindo política nativa, janela visível e `Ready`.
- Gates: 119 testes de frontend, build multipágina e contratos TypeScript, `cargo test --workspace`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, teste de processo real e `git diff --check` concluídos com sucesso.
- Limites preservados: corrida de publicação durante criação fica no ticket 03; Salvamento fica no ticket 07; Recuperação, foco por alias e resolução interativa completa de Cópia externa continuam fora deste corte.
