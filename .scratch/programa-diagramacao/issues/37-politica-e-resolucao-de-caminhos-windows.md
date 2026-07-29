# 37 — Política e resolução de caminhos Windows

**What to build:** implementar um módulo Rust compartilhado que trate caminhos locais, UNC, unidades mapeadas e caminhos longos de forma nativa, consistente e segura para todos os processos do MyAlbuns.

**Blocked by:** 01 — Plataforma e arquitetura.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [ADR 0007 — caminhos Windows e identidade física](../../../docs/adr/0007-tratar-caminhos-windows-e-identidade-fisica.md); [resolução e política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [armazenamento local e Cache](../../../docs/design/0010-armazenamento-local-e-cache.md); [ADR 0006 — publicação com transação limitada](../../../docs/adr/0006-publicar-exportacao-com-transacao-limitada.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] Implementar `AppPaths` para descobrir uma vez por processo as Known Folders e construir exatamente `%APPDATA%\MyAlbuns` para Configurações/Layout e `%LOCALAPPDATA%\MyAlbuns` para Cache, Recuperação, Estado e Logs.
- [ ] Implementar um `OperationPathContext` vazio para o proprietário acumular bindings durante o planejamento, congelá-lo em um `RootBindingPlan` imutável antes do trabalho distribuído e construir os contextos participantes somente leitura a partir desse plano; descartar tudo em sucesso, falha ou cancelamento.
- [ ] Tornar o plano transportável pela IPC e impedir que um worker resolva silenciosamente uma raiz externa ausente; ele devolve `UnboundRoot` ao proprietário para novo planejamento.
- [ ] Manter bindings fixos enquanto a tentativa estiver ativa; qualquer falha a encerra, e `Tentar novamente` explícito ou retomada após reinício cria outro contexto e captura bindings atuais, sem repetição automática de ação final.
- [ ] Aceitar somente caminhos externos totalmente qualificados de disco, UNC, unidade mapeada, VerbatimDisk e VerbatimUNC; rejeitar relativo, `.\`, `..\`, `\raiz-sem-unidade`, `C:relativo`, `Verbatim` genérico, namespace de dispositivo, curinga, fluxo alternativo e componente reservado.
- [ ] Depois da abertura, confirmar objeto de disco e tipo esperado: arquivo regular para Projeto, mídia e executável, ou diretório para origem, Destino e pasta escolhida.
- [ ] Para alvo novo, abrir e validar o diretório pai, derivar o filho com segurança e confirmar pelo handle, depois da criação, o tipo esperado e a contenção sob o pai.
- [ ] Representar caminhos com os tipos nativos do sistema e valores opacos; não usar `String`, caixa textual, canonicalização ou forma exibida como identidade, autorização ou chave persistente.
- [ ] Preservar a forma escolhida pelo usuário e capturar o binding operacional da unidade mapeada durante cada tentativa; remapeamento posterior nunca redireciona o trabalho iniciado, sem prometer a identidade do servidor físico por trás de DFS, DNS ou SMB.
- [ ] Expor comparação física de objetos existentes com `Same`, `Different` e `Indeterminate`, usando handles e as melhores informações disponíveis no Windows; erro, evidência fraca ou limitação do servidor nunca vira `Different`.
- [ ] Limitar este módulo a retornar a evidência tri-state sem política embutida; os tickets 02 e 13 implementam no guardião de abertura o bloqueio final e a falha fechada diante de identidade inconclusiva.
- [ ] Traduzir falhas para resultados tipados no mínimo equivalentes a caminho inválido, namespace não suportado, tipo de objeto inesperado, raiz não vinculada ao plano, ausente, acesso negado, indisponível temporariamente, identidade inconclusiva, conflito e outra falha de I/O.
- [ ] Produzir `Arquivo ausente` somente quando a raiz estiver confirmadamente acessível e o alvo retornar inexistente; indisponibilidade ou acesso negado preserva o vínculo e usa o estado de domínio `Arquivo indisponível`, mantendo o motivo técnico distinto para diagnóstico e ação.
- [ ] Garantir que nenhuma chamada que possa alcançar rede execute na thread da interface e que o frontend não receba acesso arbitrário ao sistema de arquivos.
- [ ] Derivar filhos apenas por componentes relativos validados, rejeitando nova raiz, `.`, `..`, namespace, fluxo alternativo, nome reservado e escape por reparse point.
- [ ] Derivar e reservar a preparação dentro da própria pasta de Destino; `ExportPipeline` possui seu ciclo de vida, enquanto `Publisher` possui promoção e limpeza de órfãos, verifica capacidade atômica no uso e nunca infere rollback integral a partir do staging.
- [ ] Manter todo Cache em `%LOCALAPPDATA%\MyAlbuns\Cache\{project-id}` independentemente da localização remota do Projeto, mídia ou Destino.
- [ ] Encapsular crates e APIs concretas atrás da interface para que `directories`, `same-file`, `dunce` ou bindings do Windows possam ser substituídos sem alterar o domínio.
- [ ] Integrar o módulo às fronteiras reais do host produzido no ticket 01 — diálogo nativo, abertura de caminho e retorno tipado à interface — e validar pelo fluxo do aplicativo pelo menos um caminho local, um UNC e uma falha indisponível, sem criar uma tela diagnóstica permanente.
- [ ] Cobrir em testes Windows caminhos locais não ASCII, UNC, unidade mapeada e seu alias, remapeamento durante operação, VerbatimDisk, VerbatimUNC, mais de 260 caracteres, curingas, ADS, reservados, arquivo/diretório incorreto, rede offline, acesso negado, alvo ausente, identidade inconclusiva, mesmo plano no host e Processador, plano novo após retomada, Cache local com mídia remota e staging no Destino.

## Comments

- 2026-07-28: a pesquisa não normativa que sustenta as escolhas substituíveis está em `docs/research/0004-caminhos-windows-e-unc.md`.
- 2026-07-28: apesar da numeração aditiva, este é um ticket de fundação e deve ser executado logo depois do ticket 01, antes dos tickets 02 e 04 que o declaram como bloqueador.
- 2026-07-29: a fundação de `AppPaths` usa temporariamente `MyAlbuns2` nas duas Known Folders para isolar os dados da versão anterior. O critério permanece com `MyAlbuns` como destino final e só pode ser encerrado depois da remoção explícita do sufixo e da definição da migração aplicável.
