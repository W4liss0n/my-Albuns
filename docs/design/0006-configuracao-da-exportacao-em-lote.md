---
status: accepted
document: design
---

# Configuração da Exportação em lote

## Objetivo

A Exportação em lote começa em uma janela dedicada aberta pela Tela de Boas-vindas. Essa janela reúne a seleção e a análise inicial dos Projetos antes de bloquear o restante do aplicativo.

## Campos

A janela contém:

- `Pasta de origem`, onde os Projetos são descobertos recursivamente;
- `Formato`, com JPEG, PNG ou PDF;
- `Modo`, com `Por lâmina` ou `Por página`;
- `Destino`, com o padrão próprio de cada Projeto ou uma raiz alternativa;
- quantidade de Projetos encontrados na origem.

Origem e Destino aceitam os caminhos totalmente qualificados da [política de caminhos](0011-resolucao-e-politica-de-caminhos.md). Durante descoberta e pré-validação, o proprietário reutiliza um único `OperationPathContext`; depois de conhecer as raízes necessárias, congela-o em um `RootBindingPlan` usado por todos os processos no processamento serial. Contexto e plano são descartados em qualquer estado terminal. Retomar depois de reiniciar cria outra tentativa e captura bindings atuais.

Não existem `Intervalo de Lâminas` ou slider de Qualidade. Todo Projeto é exportado integralmente, e JPEG usa qualidade máxima.

```text
┌──────────────────────────────────────────────────────────────────┐
│  Exportação em lote                                              │
├──────────────────────────────────────────────────────────────────┤
│  Pasta de origem   [ caminho ]                       [ Escolher ] │
│                                                                  │
│  Formato           JPEG | PNG | PDF                              │
│  Modo              Por lâmina | Por página                       │
│  Destino           Padrão de cada Projeto | Raiz alternativa     │
│                    [ caminho alternativo ]           [ Escolher ] │
│                                                                  │
│  42 Projetos encontrados                                         │
├──────────────────────────────────────────────────────────────────┤
│                              Cancelar   Verificar e exportar      │
└──────────────────────────────────────────────────────────────────┘
```

## Verificação

`Verificar e exportar` analisa os Projetos encontrados antes de adquirir o Modo de lote exclusivo.

Se houver placeholders, originais ausentes, originais indisponíveis ou outro problema reconhecido, a [Tela de Problemas](0005-tela-de-problemas.md) é aberta no contexto do lote. O usuário corrige, tenta novamente ou ignora explicitamente cada Projeto e confirma `Continuar Exportação`.

Correções criativas abertas pelo diagnóstico precisam ser salvas antes de uma nova verificação; relinks individuais ou globais do próprio lote permanecem mapas temporários da execução. Imediatamente antes do snapshot de cada item, o núcleo compartilhado reabre o arquivo, compara sua revisão ou hash com a versão pré-validada e repete a validação se houver mudança.

Quando não houver pendências sem decisão, a operação passa pelos conflitos de destino já definidos e só então adquire um `OperationLease` com a concessão `BatchExclusive`, entrando no Modo de lote exclusivo com o [Progresso de operação](0007-progresso-de-operacoes.md) compartilhado pelo aplicativo. O mecanismo é o mesmo usado pela Exportação normal, mas a instância do lote permanece única e contínua do início ao terminal de toda a tentativa; ela oferece a mesma garantia de devolver concessão, pausa do Cache e Processador. Seu contrato está em [Propriedade de estado e módulos do núcleo](0012-propriedade-de-estado-e-modulos-do-nucleo.md).

O `BatchRunner` possui descoberta, pré-validação, ordem serial e checkpoint. Para cada item conhecido, ele usa a etapa de planejamento do mesmo `ExportPipeline` da Exportação normal, congela o plano de caminhos depois de reunir as raízes e então executa cada item serialmente. O MVP processa um Projeto por vez, sem calibração ou paralelismo entre Álbuns.
