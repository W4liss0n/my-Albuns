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

Quando não houver pendências sem decisão, a operação passa pelos conflitos de destino já definidos e só então adquire um `OperationLease`, entrando no Modo de lote exclusivo com o [Progresso de operação](0007-progresso-de-operacoes.md) compartilhado pelo aplicativo. O mecanismo é o mesmo usado pela Exportação normal, mas a instância do lote permanece única e contínua do início ao terminal de toda a tentativa; ela oferece a mesma garantia de devolver concessão, pausa do Cache e Processador. Seu contrato está em [Propriedade de estado e módulos do núcleo](0012-propriedade-de-estado-e-modulos-do-nucleo.md).

O `BatchRunner` possui descoberta, pré-validação, ordem serial e checkpoint. Para cada item conhecido, ele usa a etapa de planejamento do mesmo `ExportPipeline` da Exportação normal, congela o plano de caminhos depois de reunir as raízes e então executa cada item serialmente. O MVP processa um Projeto por vez, sem calibração ou paralelismo entre Álbuns.

## Conflitos e interrupção

A confirmação de conflitos segue o refinamento aceito para Exportação: aviso genérico com `Ignorar`, `Substituir` ou `Cancelar`, sem listar arquivos. `Ignorar` preserva os arquivos existentes e exporta somente os que faltam. Nesse caso, a limpeza de órfãos não acontece. Um Projeto cujas saídas já existem por completo aparece como ignorado no resultado.

Depois de uma interrupção, `Retomar` reabre os Projetos persistidos, descarta os mapas de Religação anteriores e exige uma nova confirmação para executar. Projetos concluídos ou explicitamente ignorados não são repetidos. Fechar a janela conserva o checkpoint; `Encerrar` remove esse registro e preserva as saídas publicadas. A preparação abandonada é descartada somente quando o encerramento de seu Processador está confirmado.

O checkpoint usa `AppPaths.recovery_dir()/Batches`, respeitando o namespace de desenvolvimento `MyAlbuns2` enquanto essa separação estiver vigente. Conserva apenas opções, estados dos itens e identificação da preparação que poderá precisar de limpeza. Nunca conserva bindings de raiz, mapas de Religação nem imagens parcialmente renderizadas.

### Falta real de espaço

A falta real de espaço durante a preparação, gravação ou publicação pausa o lote antes do próximo Projeto. Não é registrada como falha de um item na Tela de Problemas, e não há bloqueio por estimativa de espaço necessário.

O progresso dá lugar a um modal compacto de `Espaço insuficiente`, com `Retomar` e `Cancelar`. O Álbum interrompido permanece pendente; os Álbuns concluídos não são repetidos. Depois de liberar espaço, `Retomar` faz nova verificação e exige confirmação para executar, incluindo o tratamento de conflitos de saídas que já existam. A publicação continua atômica por arquivo; a retomada não pressupõe desfazer arquivos já publicados.

Durante essa pausa, a janela mantém o estado e as Religações temporárias em memória. A retomada recaptura os bindings. Se o disco cheio também impedir atualizar o checkpoint, o registro anterior permanece íntegro e a janela conserva o progresso mais recente. Após fechar o aplicativo, valem o último checkpoint gravado e as regras de recuperação acima. `Cancelar` encerra esse lote e preserva as saídas já publicadas.

## Propriedade da execução

O Global mantém a execução mesmo se a interface deixar de responder. Antes de iniciar, serializa novas aberturas, adquire o `OperationGate` e solicita a pausa aos hosts dos Projetos. Cada host termina comandos já aceitos, pausa seu `CacheEngine`, reserva seu Processador e confirma o bloqueio de suas janelas. A execução só começa depois dessas confirmações, sob um único `OperationLease` do Global.

As reservas entre processos usam posses do sistema operacional. O desaparecimento do proprietário libera o modo exclusivo, e os hosts devolvem a pausa e a interação. A janela de progresso tem largura própria, independente da configuração. Ao terminar, a configuração prepara o resultado antes de reaparecer; não há linha temporária de preparação.
