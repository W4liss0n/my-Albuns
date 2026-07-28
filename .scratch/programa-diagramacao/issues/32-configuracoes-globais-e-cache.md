# 32 — Configurações globais e Cache

**What to build:** criar a janela global de Configurações em abas e entregar uma gestão simples e segura do Cache, sem limites automáticos, calibração ou limpeza de dados pertencentes a Projetos ativos.

**Blocked by:** 03 — Mídias externas e Cache; 05 — Arquitetura de UI e interação.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [Configurações do aplicativo](../../../docs/design/0009-configuracoes-do-aplicativo.md); [armazenamento local e Cache](../../../docs/design/0010-armazenamento-local-e-cache.md); [resolução e política de caminhos](../../../docs/design/0011-resolucao-e-politica-de-caminhos.md); [propriedade de estado e módulos do núcleo](../../../docs/design/0012-propriedade-de-estado-e-modulos-do-nucleo.md).

- [ ] Abrir uma janela global única de Configurações com as abas iniciais `Desempenho` e `Photoshop`, acessível pela Boas-vindas e por `Ferramentas > Configurações`.
- [ ] Manter preferências e estado local fora dos arquivos de Projeto, de seu Salvamento e de Undo/Redo; usar `SettingsStore` e `StateStore` concretos, compartilhando somente primitivas mecânicas de escrita atômica.
- [ ] Usar as Known Folders descobertas pelo módulo compartilhado e manter exatamente Configurações/Layout em `%APPDATA%\MyAlbuns` e Cache/Recuperação/Estado/Logs em `%LOCALAPPDATA%\MyAlbuns`.
- [ ] Em `Desempenho`, mostrar informações de diagnóstico relevantes ao Canvas, uso total do Cache, volume liberável de Projetos fechados e pouco espaço livre, sem calibração ou controles de processos, threads e memória no MVP.
- [ ] Oferecer `Liberar espaço` para remover integralmente somente o Cache de Projetos fechados, após informar o volume liberável e receber confirmação.
- [ ] Antes de remover um namespace fechado, reservá-lo atomicamente e desistir se ele adquirir proprietário/processo ativo; nunca confiar apenas no estado exibido pela interface.
- [ ] Não limpar nem substituir ao vivo o Cache de Projeto aberto ou com processo ativo, inclusive quando o usuário solicita liberação de espaço.
- [ ] Oferecer `Limpar todo o Cache` com confirmação. Executar imediatamente somente quando não houver Projeto nem processo relacionado ativo; caso contrário, oferecer agendar a limpeza para a próxima inicialização do aplicativo.
- [ ] Fazer `CacheEngine` possuir jobs, índice, manutenção e reservas; a Limpeza total adquire `CacheMaintenance` no `OperationGate` e libera a concessão em sucesso, falha ou cancelamento.
- [ ] Na inicialização agendada, limpar antes de abrir Projetos ou iniciar Processadores; cancelar ou falhar preserva Projetos, vínculos e Arquivos originais.
- [ ] Ambas as ações removem somente representações e metadados descartáveis e permitem reconstrução sob demanda; Cache nunca se torna fonte válida de Exportação nem mascara original ausente.
- [ ] Projeto, mídia ou Destino em UNC ou unidade mapeada nunca desloca o Cache para a rede nem cria um cache persistente de resolução de caminhos.
- [ ] Não impor limite rígido, expiração por idade, patamares progressivos ou exclusão automática por tamanho no MVP.
- [ ] Exibir aviso amigável de pouco espaço livre quando o aplicativo detectar risco real para uma operação, com `Liberar espaço` e `Agora não`; a exibição nunca remove conteúdo por si.
- [ ] Se uma representação não puder ser publicada por falta de espaço, interromper somente sua geração e manter Projeto e original intactos.
- [ ] Usar o progresso global simples nas limpezas bloqueantes e impedir novas escritas no Cache enquanto a operação possuir a reserva necessária.
- [ ] Testes cobrem Cache vazio, sem conteúdo liberável, Projeto fechado, Projeto ativo, corrida de abertura, pouco disco, agendamento para próxima inicialização, sucesso, cancelamento e falha.

## Comments

- 2026-07-28: calibração, perfis automáticos e avisos progressivos por 2/4/8/16 GB foram retirados do MVP até existirem medições que justifiquem essa complexidade.
