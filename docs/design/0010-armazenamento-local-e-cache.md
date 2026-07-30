---
status: accepted
document: design
---

# Armazenamento local e Cache

## Objetivo

Separar dados valiosos do usuário, estado local da máquina, Recuperação e Cache descartável. Projetos e Exportações permanecem nos locais escolhidos pelo usuário; nenhuma pasta do aplicativo substitui o arquivo do Projeto ou os Arquivos vinculados originais.

Esta é a estrutura mínima da primeira versão. Novas categorias só devem ser criadas quando uma necessidade medida não couber nas existentes.

A descoberta das raízes, o suporte a caminhos Windows e os contextos temporários de cada operação pertencem a [Resolução e política de caminhos](0011-resolucao-e-politica-de-caminhos.md). Este documento possui somente a organização dos dados depois que essas raízes foram resolvidas.

A propriedade lógica dos stores e do `CacheEngine` segue [Propriedade de estado e módulos do núcleo](0012-propriedade-de-estado-e-modulos-do-nucleo.md).

## Raízes do aplicativo

> **Namespace transitório de desenvolvimento (2026-07-29):** enquanto a
> nova geração do programa não estiver concluída, a implementação usa
> `MyAlbuns2` nas duas raízes (`%APPDATA%\MyAlbuns2` e
> `%LOCALAPPDATA%\MyAlbuns2`). Essa separação impede que o desenvolvimento
> leia, sobrescreva ou misture automaticamente os dados da versão anterior,
> que permanecem em `MyAlbuns`. A árvore normativa abaixo continua sendo o
> destino final; a remoção do sufixo `2` e qualquer migração de dados exigem
> uma alteração explícita antes da distribuição final.

```text
%APPDATA%\MyAlbuns\
├── settings.json
└── Layouts\

%LOCALAPPDATA%\MyAlbuns\
├── Cache\
│   └── {project-id}\
│       ├── metadata.json
│       └── Media\
├── Recovery\
│   ├── Projects\
│   │   └── {project-id}.json
│   └── Batches\
│       └── {batch-id}.json
├── State\
│   ├── recent-projects.json
│   └── photoshop.json
└── Logs\
```

As duas raízes são obtidas pelas pastas conhecidas do Windows e nunca pelo diretório corrente do processo. `%APPDATA%` contém preferências e conteúdo global criado pelo usuário. `%LOCALAPPDATA%` contém dados ligados à máquina, diagnóstico, Recuperação e conteúdo reconstruível.

## Stores concretos

Cada categoria conserva suas próprias garantias:

- `SettingsStore` grava `settings.json`;
- `LayoutCatalogStore` grava o catálogo em `Layouts`;
- `StateStore` grava os arquivos de `State`;
- `RecoveryStore` grava checkpoints de Projetos e lotes;
- `ProjectStore` grava o arquivo de Projeto no local escolhido pelo usuário;
- `CacheEngine` possui `metadata.json`, artefatos e manutenção do Cache.

Eles podem reutilizar primitivas internas para criar temporário irmão, descarregar buffers, substituir um único arquivo, versionar envelopes e traduzir erros. Não existe um `AppStorage` ou `Store<T>` genérico que iguale políticas de corrupção, recuperação, concorrência e ciclo de vida diferentes.

## Dados globais

`SettingsStore` guarda preferências de apresentação em `settings.json`. `LayoutCatalogStore` guarda o catálogo global criado pelo usuário em `Layouts`. Esses dados não são Cache e nunca podem ser apagados por uma ação de liberação de espaço.

Alterações globais usam schema e substituição atômica. Janelas ou sessões consultam a revisão vigente ao abrir, receber foco ou solicitar atualização manual. Broadcast imediato entre todas as Janelas pode ser acrescentado depois se a topologia escolhida torná-lo praticamente gratuito, mas não é requisito do MVP.

`StateStore` mantém em `State` informações locais independentes, que não fazem sentido fora desta máquina: Projetos recentes, a instalação escolhida do Photoshop e as preferências de interface que dependem da tela.

A divisão entre as duas raízes segue o que a preferência representa:

| Preferência | Raiz | Motivo |
|---|---|---|
| Altura e visibilidade do Painel de imagens | `%LOCALAPPDATA%` | proporção ajustada a um monitor |
| Largura e visibilidade do Painel contextual | `%LOCALAPPDATA%` | proporção ajustada a um monitor |
| Tamanho das miniaturas, por aba | `%LOCALAPPDATA%` | densidade ajustada a uma resolução |
| Seções recolhidas, por contexto | `%LOCALAPPDATA%` | acompanha o espaço vertical disponível |
| Ordenação, por aba | `%APPDATA%` | hábito de trabalho, independente da tela |
| Filtro de uso, por aba | `%APPDATA%` | hábito de trabalho, independente da tela |

Geometria de painel carregada para uma tela menor chega errada e obriga o usuário a refazê-la; por isso ela fica na máquina. Ordenação e Filtro de uso descrevem como a pessoa trabalha e acompanham seu perfil.

Nenhuma dessas preferências altera o Projeto, participa de Undo/Redo ou exige Salvamento. Perdê-las é irrelevante: a próxima sessão começa nos padrões.

`Logs` permanece local; sua retenção será definida quando houver dados reais de diagnóstico.

## Recuperação

`RecoveryStore` mantém `Recovery\Projects\{project-id}.json` como um checkpoint atômico com:

- schema e Identidade;
- estado criativo consolidado ainda não salvo;
- marcador da revisão persistida da qual ele deriva.

O checkpoint não contém pixels, originais ou pilhas de Undo/Redo. Uma sessão recuperada abre marcada como alterada e com Histórico vazio; o Histórico da sessão normal continua existindo apenas enquanto ela permanece viva. `Salvar` ou descartar a recuperação remove o checkpoint correspondente.

Uma Cópia externa recebe nova Identidade antes de consultar Recuperação ou Cache. Se essa identidade não puder ser persistida, nenhuma pasta da identidade duplicada é montada.

`Recovery\Batches\{batch-id}.json` contém somente:

- schema e identidade da execução;
- opções e plano ordenado;
- estado de cada item: pendente, concluído, ignorado ou falho;
- identificação do item que estava em execução.

Não contém estado criativo nem preparação parcial. Após interrupção, o item que estava em execução volta a `pendente` e é refeito integralmente; itens já concluídos não são repetidos.

## Namespace do Projeto

Cada pasta abaixo de `Cache` usa uma representação opaca e segura da Identidade persistente, indicada por `{project-id}`. Nome e caminho do arquivo não participam desse namespace.

- mover ou renomear preserva a pasta;
- `Salvar como` começa em uma pasta nova e vazia;
- uma Cópia externa recebe outra pasta;
- Projetos diferentes nunca compartilham estado mutável de Cache.

O formato textual definitivo da Identidade pertence ao documento de Projeto. Para o Cache, ela precisa ser estável, única e segura como nome de diretório.

## Conteúdo mínimo

```text
Cache\
└── {project-id}\
    ├── metadata.json
    └── Media\
        ├── {media-id}.{generation-id}.{formato}
        └── {media-id}.{generation-id}.tmp
```

O baseline contém uma única representação visual reduzida por Foto ou Decorativo. A mesma representação atende ao Painel e ao Canvas; miniaturas de Lâmina podem ser montadas em memória. Não existem tiles, pirâmides nem previews persistidos de Lâmina no MVP, salvo se o spike provar com medições que a representação única não atende.

Conteúdo opaco usa JPEG; conteúdo que precisa preservar transparência usa PNG.
O formato é propriedade do artefato derivado e integra seu caminho e o índice
do Cache. Essa escolha não altera o original nem permite que a Exportação use a
representação reduzida.

O `RootBindingPlan` e os contextos locais que reutilizam uma raiz durante Importação, Cache ou Exportação existem somente em memória e não criam outra pasta, índice ou categoria sob `Cache`.

`CacheEngine` possui os jobs, o índice e as gerações. Cada job grava um temporário próprio, verifica se o pedido e o original ainda são atuais e promove o artefato imutável antes de publicar a entrada correspondente em `metadata.json`. Uma queda pode deixar temporários ou gerações não referenciadas, que são descartados no próximo uso, sem fazer o índice apontar para um arquivo incompleto.

## Metadados

`metadata.json` é um índice descartável e versionado. Ele mantém somente o necessário para localizar e validar a representação:

- schema e versão da representação;
- Identidade do Projeto e último uso;
- `mediaId`, `generationId` e nome do artefato;
- dimensões, formato e orientação EXIF conhecidos;
- tamanho e datas do original;
- fingerprint versionado.

Identidade da mídia, caminho original, categoria e decisões do usuário pertencem ao Projeto. Ausência ou corrupção do índice exige reconstrução, nunca perda de conteúdo.

O fingerprint é um conjunto versionado de evidências do estado do original, não necessariamente um hash completo. O algoritmo exato permanece adiado.

## Invalidação e propriedade

O Monitor apenas sinaliza uma possível mudança e agrupa eventos. Depois de uma nova inspeção confirmar alteração estável, divergência de tamanho ou data, reaparecimento, Religação, versão incompatível ou artefato inválido, o `CacheEngine` invalida somente a mídia afetada. Pan, Zoom, Frame e Layout não invalidam a representação da fonte.

É aceito no MVP o caso raro de uma alteração feita com o aplicativo fechado conservar exatamente tamanho e data. A Exportação reabre o original e não depende dessa concessão.

Fora de manutenção, `CacheEngine` é o proprietário lógico de cada namespace. Conforme a topologia escolhida, um único Processador de Imagens atua como adaptador escritor dos arquivos. Jobs equivalentes podem ser agrupados e obsoletos cancelados. Cache não participa de Salvamento, Undo/Redo ou Recuperação.

## Liberação de espaço

Não existe limite rígido, expiração automática por idade ou sequência de alertas por tamanho. O aplicativo mostra o total ocupado e avisa quando o espaço livre do volume estiver baixo.

`Liberar espaço`:

- calcula previamente o volume removível;
- reserva atomicamente cada namespace sem proprietário ativo;
- remove somente Cache de Projetos fechados;
- preserva a pasta se não conseguir a reserva.

`Limpar todo o Cache` executa imediatamente apenas quando não houver Projeto ou Processador ativo e depois de adquirir a concessão `CacheMaintenance` do `OperationGate`. Caso contrário, o usuário pode agendá-lo para a próxima inicialização, antes da abertura de Projetos. A concessão impede abertura, Processador ou Exportação concorrente e é liberada em sucesso, falha ou cancelamento. O MVP não pausa editores nem remove Cache ativo ao vivo.

Nenhuma ação de Cache remove Projetos, itens do Painel, vínculos, Recuperação, Layouts, preferências, Exportações ou originais.

## Cenários de validação

| Cenário | Resultado esperado |
|---|---|
| Projeto renomeado ou movido | mesma Identidade e mesmo namespace |
| `Salvar como` | nova Identidade e namespace vazio |
| Cópia externa gravável | nova Identidade antes de Cache ou Recuperação |
| Cópia externa somente leitura | falha fechada, sem montar namespace duplicado |
| original alterado | somente sua representação é invalidada |
| original ausente | representação pode permanecer como contexto, mas não autoriza Exportação |
| origem de rede indisponível | vínculo e última representação são preservados como indisponíveis, sem confirmar ausência |
| índice corrompido | índice descartado e reconstruído |
| job obsoleto termina | geração descartada |
| queda durante geração | temporário descartado; geração publicada anterior continua válida |
| Projeto abre durante `Liberar espaço` | namespace reservado por quem vencer; nunca remoção concorrente |
| limpeza total com Projeto ativo | agendada para a próxima inicialização |
| Exportação | usa snapshot validado e originais; Cache não é fonte final |
| Projeto ou mídia em UNC | Cache continua sob a raiz local do aplicativo e a Identidade do Projeto |

## Decisões adiadas

- formato e resolução da representação reduzida;
- necessidade de tiles depois do spike;
- algoritmo concreto do fingerprint;
- retenção de Logs;
- representação textual da Identidade do Projeto.
