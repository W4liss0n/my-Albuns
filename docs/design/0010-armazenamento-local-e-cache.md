---
status: accepted
document: design
updated: 2026-09-05
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

O baseline contém uma única representação visual reduzida por Foto ou Decorativo. A mesma representação atende ao Painel e ao Canvas; miniaturas de Lâmina podem ser montadas em memória. A [medição reproduzível do Programa 03A](../research/0032-representacao-reduzida-e-politica-de-decode.md) confirmou a representação única e rejeitou tiles, pirâmides e previews persistidos de Lâmina no MVP.

Cada representação preparada é entregue à interface imediatamente, sem esperar
as demais mídias da demanda. O Painel mantém em pré-carga o último trecho
observado de cada aba, incluindo a margem de `122 px` acima e abaixo da área
visível. Alternar Fotos e Decorativos não descarta esse trecho; rolar, filtrar,
remover mídias ou fechar o Painel atualiza a demanda e libera o que saiu dela.
Essa retenção não abrange todo o catálogo nem acumula trechos de rolagens
anteriores. Depois da observação do Monitor, uma representação ainda residente
e vinculada à mesma origem pode ser entregue novamente sem executar outro job;
mudanças confirmadas continuam revogando a representação anterior.

O maior lado mede no máximo `1.600 px`. Conteúdo opaco usa JPEG qualidade `84`; conteúdo que precisa preservar transparência usa PNG RGBA.
O formato é propriedade do artefato derivado e integra seu caminho e o índice
do Cache. Essa escolha não altera o original nem permite que a Exportação use a
representação reduzida. Ambos os formatos carregam o perfil canônico
`sRGB2014.icc`; a orientação EXIF/TIFF é aplicada exatamente uma vez antes da
redução.

O Processador aceita JPEG, PNG e TIFF de uma página e recusa TIFF multipágina.
Antes de materializar o raster, impõe `134.217.728` pixels e `512 MiB` de
alocação planejada do decoder. Esses valores, os codecs e as variantes de cor
aceitas pertencem à política versionada da representação e são rastreáveis ao
spike; não são inferidos novamente por cada chamador.

O `RootBindingPlan` e os contextos locais que reutilizam uma raiz durante Importação, Cache ou Exportação existem somente em memória e não criam outra pasta, índice ou categoria sob `Cache`.

`CacheEngine` possui os jobs, o índice e as gerações. Cada job grava um temporário próprio, verifica se o pedido e o original ainda são atuais e promove o artefato imutável antes de publicar a entrada correspondente em `metadata.json`. Uma queda pode deixar temporários ou gerações não referenciadas, que são descartados no próximo uso, sem fazer o índice apontar para um arquivo incompleto.

A preparação de imagens do Projeto é compartilhada por importação, Religação,
nova tentativa de leitura e mudanças de edição ou Histórico que introduzam ou
passem a usar outra imagem. Essas ações aguardam a origem validada e o Cache
completo antes de terminar, reutilizando uma geração válida. O progresso
`Processando Imagens — X de Y` conta a conclusão conjunta de cada imagem; uma
falha de Cache gera um problema sem remover um vínculo válido. Salvar e Fechar
aguardam a mesma fila de ações. Jobs necessários a essas ações sobrevivem à
mudança de área visível, mas continuam sujeitos à obsolescência da origem e da
Identidade. Artefatos de imagens fora da área visível ficam no disco; a preparação
do lote não torna todas as prévias residentes em memória.

A importação conserva temporariamente a evidência da inspeção de cada Foto
validada. A primeira atualização do Monitor pode adotá-la sem decodificar o
Original outra vez quando caminho, tipo, identidade física, tamanho e datas
observadas ainda correspondem. A evidência é consumida nessa adoção; alteração
da origem ou impossibilidade de comprovar essa correspondência exige a inspeção
normal. Essa adoção não cria Histórico nem altera o estado salvo do Projeto.

Ao concluir uma preparação solicitada por ação, `CacheEngine` conserva um
resultado pequeno em memória: a observação da origem, o artefato publicado e o
SHA-256 dos mesmos bytes reduzidos que o Host decodificou e validou. A primeira
demanda pode consumir esse resultado sem iniciar outro Processador. Ela confere
a demanda atual, a observação da origem, a entrada do índice e o hash dos bytes
que serão publicados no registro de prévias. Corrupção, mudança de origem ou
vínculo, retirada da Identidade e ausência do índice impedem esse reuso.

O resultado não contém pixels do Original nem bytes de prévias fora da área
visível. A reconciliação do catálogo descarta resultados de mídias removidas;
invalidações e novos trabalhos retiram os resultados anteriores. O resultado é
consumido pela primeira demanda válida, e as demandas seguintes usam a política
de residência existente. Essa passagem dentro da Sessão aproveita uma geração
já verificada pelo fingerprint integral; não autoriza reuso de um índice apenas
por tamanho ou data e não acrescenta campos ao índice persistido.

Em Novo Projeto, a identidade ainda não existe durante a escolha de uma imagem
decorativa. A seleção só retorna depois da leitura e decodificação completas; o
registro provisório guarda os bytes codificados dessa prévia em memória,
preservando orientação e perfil de cor. Sua liberação descarta os bytes, sem
criar um arquivo ou namespace de Projeto. A criação revalida o Original e o
Host prepara o Cache canônico após assumir a identidade, antes de liberar a
Janela do Projeto. Problemas nessa preparação preservam o Projeto criado e
aparecem como avisos na janela.

## Metadados

`metadata.json` é um índice descartável e versionado. Ele mantém somente o necessário para localizar e validar a representação:

- schema e versão da representação;
- Identidade do Projeto e último uso;
- `mediaId`, marca opaca SHA-256 do caminho lógico que originou a geração,
  `generationId` e nome do artefato;
- dimensões, formato, orientação EXIF e quantidade de páginas quando aplicável;
- perfil de cor básico (`srgb` nesta política medida);
- tamanho e datas do original;
- fingerprint versionado.

Identidade da mídia, caminho original, categoria e decisões do usuário pertencem ao Projeto. A marca opaca não serializa o caminho textual: ela apenas impede que uma geração produzida depois de Religação seja reutilizada quando Undo, Descartar ou Recuperação restaurarem outro vínculo para o mesmo `mediaId`. Ausência ou corrupção do índice exige reconstrução, nunca perda de conteúdo.

O fingerprint v1 é `sha256-full-file-v1`: SHA-256 dos bytes integrais abertos
pelo Processador, acompanhado pelo tamanho e pelas datas de criação e
modificação disponíveis no filesystem. Tamanho e datas permitem ao Monitor
confirmar mudanças comuns; não substituem o hash integral nem autorizam pixels
finais.

## Invalidação e propriedade

O Monitor apenas sinaliza uma possível mudança e agrupa eventos. Depois de uma nova inspeção confirmar alteração estável, divergência de tamanho ou data, reaparecimento, Religação, versão incompatível ou artefato inválido, o `CacheEngine` invalida somente a mídia afetada. Pan, Zoom, Frame e Layout não invalidam a representação da fonte.

Ao confirmar uma atualização automática da mesma origem, o Monitor invalida o
reuso da prévia anterior, preservando seus bytes apenas para apresentação até a
publicação verificada da sucessora. Não há diálogo de progresso nesse fluxo.
Essa retenção não transfere pixels entre vínculos, Projetos ou Identidades e
permanece limitada à demanda residente do Painel e do Canvas.

É aceito no MVP o caso raro de uma alteração feita com o aplicativo fechado conservar exatamente tamanho e data. A Exportação reabre o original e não depende dessa concessão.

Fora de manutenção, o `CacheEngine` de cada Projeto é o proprietário lógico de seu namespace. Até dois Processadores de Imagens podem preparar mídias simultaneamente, com o mesmo limite para ações do usuário e miniaturas em segundo plano. Cada processo escreve uma geração própria; a atualização do índice permanece serializada pelo `CacheEngine`, e a coleta de gerações não remove candidatos de outros trabalhos ativos. Jobs equivalentes compartilham o resultado e obsoletos são cancelados. Cada escritor publica sua instância exata em um dos dois registros duráveis do namespace antes de receber trabalho; a recuperação espera a saída de todos eles antes de qualquer limpeza. O primeiro registro conserva o nome usado pela implementação anterior.

A validação inicial dos JPEGs também usa até dois trabalhadores, sob uma reserva de toda a capacidade de processamento durante essa etapa. Os resultados são aplicados na ordem da seleção, em uma única ação do Histórico. A preparação do Cache informa conclusão por imagem, mesmo quando os resultados chegam fora de ordem, e uma falha individual não interrompe o lote. A Exportação aguarda a pausa do Cache e reserva toda a capacidade do Processador. Cache não participa de Salvamento, Undo/Redo ou Recuperação.

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

- retenção de Logs.
