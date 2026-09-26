---
status: accepted
document: design
updated: 2026-09-24
---

# Configuração da Exportação em lote

## Objetivo

A Exportação em lote começa em uma janela dedicada aberta pela Tela de Boas-vindas. Essa janela reúne a seleção e a análise inicial dos Projetos antes de bloquear o restante do aplicativo.

## Campos

A janela contém:

- `Projetos`, a pasta de origem onde os Projetos são descobertos recursivamente;
- `Formato`, com JPEG, PNG ou PDF, e a opção `Exportar como páginas simples`,
  que escolhe o modo: desmarcada corresponde à saída por lâmina; marcada, à
  saída por página;
- `Destino`, com `Padrão de cada projeto` ou `Outra pasta` como raiz
  alternativa;
- quantidade de Projetos encontrados na origem, no rodapé.

Origem e Destino aceitam os caminhos totalmente qualificados da [política de caminhos](0011-resolucao-e-politica-de-caminhos.md). Durante descoberta e pré-validação, o proprietário reutiliza um único `OperationPathContext`; depois de conhecer as raízes necessárias, congela-o em um `RootBindingPlan` usado por todos os processos no processamento serial. Contexto e plano são descartados em qualquer estado terminal. Retomar depois de reiniciar cria outra tentativa e captura bindings atuais.

Não existem `Intervalo de Lâminas` ou slider de Qualidade. Todo Projeto é exportado integralmente, e JPEG usa qualidade máxima.

```text
┌──────────────────────────────────────────────────────────────────┐
│                       Exportação em lote                      ✕  │
├──────────────────────────────────────────────────────────────────┤
│  Projetos                                                        │
│  [ caminho ]                                      [ Escolher… ]  │
│                                                                  │
│  Formato                                                         │
│  [ JPEG ▾ ]                    □ Exportar como páginas simples   │
│                                                                  │
│  Destino                                                         │
│  ● Padrão de cada projeto                                        │
│  ○ Outra pasta [ caminho alternativo ]            [ Escolher… ]  │
├──────────────────────────────────────────────────────────────────┤
│  42 projetos encontrados       Cancelar   Verificar e exportar   │
└──────────────────────────────────────────────────────────────────┘
```

### Refinamento de 24/09/2026

Decisão do autor, validada em protótipo, para seguir o mesmo padrão da
[Exportação normal](0004-exportacao-normal.md#refinamento-de-24092026):

- a barra da janela mostra somente `Exportação em lote`, sem a marca, em todas
  as fases da janela;
- blocos com títulos curtos (`Projetos`, `Formato`, `Destino`) separados apenas
  por espaço, controles de 31 pixels e rodapé em faixa de tom com a quantidade
  de Projetos encontrados; somente `Verificar e exportar` é ação principal;
- o modo usa o mesmo controle e o mesmo texto da Exportação normal,
  `Exportar como páginas simples`, na linha do formato, encostado à direita e
  alinhado com `Escolher…` e com a ação principal. O seletor `Por lâmina` /
  `Por página` deixa de existir; a opção de lote e o contrato com o núcleo
  continuam usando `sheet` e `page`;
- `Outra pasta` tem o campo e `Escolher…` na mesma linha, como o intervalo da
  Exportação normal; ambos ficam desabilitados, e vazios, enquanto
  `Padrão de cada projeto` estiver selecionado.

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

O progresso dá lugar ao modal compartilhado de `Espaço insuficiente`. O Álbum interrompido permanece pendente; os Álbuns concluídos não são repetidos. Durante a tentativa viva, `Retomar` reutiliza a preparação válida do núcleo compartilhado: gera apenas arquivos pendentes ou continua a publicação no arquivo que falhou, preservando a política de conflitos já confirmada. Não recarrega nem recompõe o Álbum quando a geração já terminou. Caso a falta de espaço tenha ocorrido antes de haver uma preparação reutilizável, faz a verificação normal. A publicação continua atômica por arquivo; a retomada não pressupõe desfazer arquivos já publicados.

Quando já houver arquivos publicados do Álbum atual, o próprio modal informa a publicação parcial e orienta a retomar para concluir. Essa frase não aparece quando a falta de espaço ocorre antes de qualquer publicação desse Álbum.

Durante essa pausa, a janela mantém o estado e as Religações temporárias em memória. A preparação reutilizável conserva os bindings da tentativa; uma execução reconstruída do checkpoint captura bindings atuais. Se o disco cheio também impedir atualizar o checkpoint, o registro anterior permanece íntegro e a janela conserva o progresso mais recente. Após fechar o aplicativo, valem o último checkpoint gravado e as regras de recuperação acima. `Cancelar` encerra esse lote e preserva as saídas já publicadas.

## Propriedade da execução

O Global mantém a execução mesmo se a interface deixar de responder. Antes de iniciar, serializa novas aberturas, adquire o `OperationGate` e solicita a pausa aos hosts dos Projetos. Cada host termina comandos já aceitos, pausa seu `CacheEngine`, reserva seu Processador e confirma o bloqueio de suas janelas. A execução só começa depois dessas confirmações, sob um único `OperationLease` do Global.

As reservas entre processos usam posses do sistema operacional. O desaparecimento do proprietário libera o modo exclusivo, e os hosts devolvem a pausa e a interação. A janela de progresso tem largura própria, independente da configuração. Ao terminar, a configuração prepara o resultado antes de reaparecer; não há linha temporária de preparação.
