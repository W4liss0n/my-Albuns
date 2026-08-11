---
status: accepted
document: design
updated: 2026-08-10
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
│   └── {project-key}\
│       ├── metadata.json
│       └── Media\
├── Recovery\
│   ├── Projects\
│   │   └── {project-key}.json
│   └── Batches\
│       └── {batch-id}.json
├── State\
│   ├── ProjectIdentities\
│   │   └── {project-key}.json
│   ├── ProjectIdentityLeases\
│   ├── WebView2\
│   │   └── {project-key}\
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
- `ProjectIdentityRegistry` grava a evidência durável por Identidade em `State\ProjectIdentities`;
- `RecoveryStore` grava checkpoints de Projetos e lotes;
- `ProjectStore` grava o arquivo de Projeto no local escolhido pelo usuário;
- `CacheEngine` possui `metadata.json`, artefatos e manutenção do Cache.

Eles podem reutilizar primitivas internas para criar temporário irmão, descarregar buffers, substituir um único arquivo, versionar envelopes e traduzir erros. Não existe um `AppStorage` ou `Store<T>` genérico que iguale políticas de corrupção, recuperação, concorrência e ciclo de vida diferentes.

## Dados globais

`SettingsStore` guarda preferências de apresentação em `settings.json`. `LayoutCatalogStore` guarda o catálogo global criado pelo usuário em `Layouts`. Esses dados não são Cache e nunca podem ser apagados por uma ação de liberação de espaço.

Alterações globais usam schema e substituição atômica. Janelas ou sessões consultam a revisão vigente ao abrir, receber foco ou solicitar atualização manual. Como as Janelas pertencem a hosts de Projeto distintos, um broadcast imediato exigiria coordenação entre processos; ele não é requisito do MVP e só será acrescentado diante de necessidade observada.

`StateStore` mantém em `State` informações locais independentes, que não fazem sentido fora desta máquina: Projetos recentes, a instalação escolhida do Photoshop e as preferências de interface que dependem da tela.

`ProjectIdentityRegistry` é um store concreto distinto dentro da mesma raiz. Sua falha não pode ser tratada como perda de uma preferência: o registro participa da autorização de Identidade antes de montar qualquer estado local de Projeto. Projetos recentes podem ser limitados, reordenados ou removidos sem apagar essa evidência.

Os dados internos do WebView2 ficam em `State\WebView2\{project-key}`. Cada
host de Projeto deriva uma chave opaca própria da Identidade persistida; hosts
distintos nunca compartilham o mesmo diretório de perfil do navegador.

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

`RecoveryStore` mantém `Recovery\Projects\{project-key}.json` como um checkpoint atômico com:

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

## Evidência local de Identidade

`ProjectIdentityRegistry` mantém um arquivo por Identidade em `State\ProjectIdentities\{project-key}.json`. Cada registro fechado e versionado contém somente:

- schema do próprio registro;
- a Identidade canônica do Projeto, para validar a chave e detectar corrupção;
- a última Localização autorizada, codificada pelo mesmo DTO reversível `windowsUtf16` do documento.

O registro não serializa pathname como identidade física nem transforma uma observação do sistema operacional em verdade eterna. Na abertura seguinte, a Localização registrada serve para o módulo de caminhos abrir novamente a instância anterior e produzir evidência física atual por handles. Enquanto a tentativa ou a Sessão estiver viva, `PersistedBaseline` e a trava do arquivo conservam a evidência física; `ProjectIdentityLease` conserva a exclusividade da Identidade e o alvo ativo autorizado. Depois do fechamento, essas posses são liberadas e somente o registro local permanece.

Cada atualização grava o registro completo por substituição atômica. Falha antes da substituição conserva o registro anterior; falha que impeça comprovar o estado final não autoriza tratar o novo valor como vigente. A ordem das transições de Projeto, a classificação da abertura e o momento em que a autoridade pode ser emitida pertencem exclusivamente ao [contrato público de persistência](0015-contrato-publico-de-persistencia-do-project-core.md).

Fechamento normal ou inesperado, remoção de Projetos recentes, `Liberar espaço` e `Limpar todo o Cache` não removem esses registros. A primeira versão não executa expiração automática. Ausência legítima significa primeira observação da Identidade nesta máquina; registro corrompido, inacessível ou incompatível falha de forma fechada e nunca é confundido com ausência.

## Namespace do Projeto

Cada pasta abaixo de `Cache` usa uma representação opaca e segura da Identidade persistente, indicada por `{project-key}`. A mesma chave identifica o checkpoint futuro em `Recovery` e o perfil do WebView2. Nome e caminho do arquivo não participam desse namespace.

- mover ou renomear preserva a pasta;
- `Salvar como` começa em uma pasta nova e vazia;
- uma Cópia externa recebe outra pasta;
- Projetos diferentes nunca compartilham estado mutável de Cache.

A Identidade é o UUID v4 canônico, minúsculo e hifenizado definido pelo
documento de Projeto. A implementação deriva `project-{sha256}` dos bytes
UTF-8 dessa representação canônica e usa a mesma chave opaca para o registro
local, Cache, Recuperação e WebView2. O valor original nunca vira componente
de caminho.

## Conteúdo mínimo

```text
Cache\
└── {project-key}\
    ├── metadata.json
    └── Media\
        ├── {media-key}.{generation-id}.{formato}
        └── {media-key}.{generation-id}.tmp
```

`{media-key}` é `media-{sha256}` derivado da Identidade da mídia. A Identidade
original continua preservada no Projeto, no protocolo e em `metadata.json`;
somente a chave opaca participa do nome do artefato.

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

Fora de manutenção, o `CacheEngine` de cada Projeto é o proprietário lógico de seu namespace. O Processador de Imagens isolado daquela Sessão atua como único adaptador escritor dos arquivos. Jobs equivalentes podem ser agrupados e obsoletos cancelados. Cache não participa de Salvamento, Undo/Redo ou Recuperação.

## Liberação de espaço

Não existe limite rígido, expiração automática por idade ou sequência de alertas por tamanho. O aplicativo mostra o total ocupado e avisa quando o espaço livre do volume estiver baixo.

`Liberar espaço`:

- calcula previamente o volume removível;
- reserva atomicamente cada namespace sem proprietário ativo;
- remove somente Cache de Projetos fechados;
- preserva a pasta se não conseguir a reserva.

`Limpar todo o Cache` executa imediatamente apenas quando não houver Projeto ou Processador ativo e depois de adquirir a concessão exclusiva única do `OperationGate`. Caso contrário, o usuário pode agendá-lo para a próxima inicialização, antes da abertura de Projetos. A concessão impede abertura, Processador ou Exportação concorrente e é liberada em sucesso, falha ou cancelamento. O MVP não pausa editores nem remove Cache ativo ao vivo.

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
- retenção de Logs.
