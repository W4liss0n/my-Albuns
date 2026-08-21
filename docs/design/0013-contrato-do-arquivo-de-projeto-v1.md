---
status: accepted
document: design
date: 2026-08-03
updated: 2026-08-21
---

# Contrato do Arquivo de Projeto v1

## Objetivo

Definir o envelope público, a fronteira persistente e o ciclo de evolução do Arquivo de Projeto sem levar JSON, I/O ou versões antigas ao `ProjectDomain`. Este design detalha o [ADR 0009](../adr/0009-adotar-arquivo-myalbuns-json-versionado.md) e incorpora o codec nativo comprovado para caminhos Windows.

Este documento permanece o contrato fechado da v1. A versão pública atual é a
[v3](0017-contrato-da-primeira-composicao-com-foto.md); o leitor conserva v1 e
a migra sequencialmente apenas em memória.

## Documento público

- a extensão é `.myalbuns`;
- o tipo apresentado pelo Windows é `Projeto MyAlbuns` e sua associação abre o arquivo diretamente no MyAlbuns;
- o conteúdo é um único JSON UTF-8, emitido sem BOM;
- o formato é legível para diagnóstico, mas edição manual não constitui uso suportado;
- espaços, quebras de linha e ordem de propriedades não participam da compatibilidade;
- a extensão seleciona o aplicativo, mas não substitui a identificação interna do documento.

A extensão participa da associação do Windows e da descoberta automática, não da validação autoritativa do conteúdo. Um caminho fornecido explicitamente por `Abrir Projeto` ou `Projetos recentes` pode abrir com outra extensão quando `documentType` e todo o documento forem válidos. A abertura direta pelo Windows associa somente `.myalbuns`; a descoberta da Exportação em lote também considera somente `.myalbuns` e reporta como inválido qualquer candidato dessa extensão cujo conteúdo não seja um Projeto.

Arquivos `.myalbum` e documentos com `schemaVersion: 3` produzidos pelos spikes são fixtures descartáveis. Eles não são uma versão anterior do formato público e não recebem importador ou migração.

## Envelope

Todo documento público possui exatamente estes campos na raiz:

```json
{
  "documentType": "myalbuns.project",
  "schemaVersion": 1,
  "projectId": "550e8400-e29b-41d4-a716-446655440000",
  "revision": 0,
  "project": {
    "document": {
      "displayUnit": "mm",
      "sheetWidthUm": 600000,
      "sheetHeightUm": 300000,
      "dpi": 300,
      "bleedUm": 3000,
      "safetyUm": 3000
    },
    "visualDefaults": {
      "background": {
        "scope": "bothSides",
        "both": {
          "kind": "color",
          "rgb": "#FFFFFF"
        }
      },
      "overlay": {
        "scope": "bothSides",
        "both": null
      },
      "frameBorder": {
        "kind": "none"
      }
    },
    "media": [],
    "sheets": [
      {
        "id": "00000000-0000-4000-8000-000000000001",
        "activeSides": "both"
      },
      {
        "id": "00000000-0000-4000-8000-000000000002",
        "activeSides": "both"
      }
    ]
  }
}
```

Esse é o fixture mínimo válido e neutro: duas Lâminas duplas, Background branco, nenhum Overlay, nenhuma borda, nenhum Frame e nenhuma mídia. Não existe mapa genérico ou extensão implícita no payload.

### `documentType`

`myalbuns.project` identifica o conteúdo. Ausência, tipo JSON diferente ou outro valor produz `InvalidDocumentType`, independentemente da extensão do arquivo.

### `schemaVersion`

É um inteiro positivo que seleciona primeiro o DTO persistente e sua cadeia de migração. O primeiro contrato público é `1`; ele não herda a numeração interna dos spikes e não se confunde com versão do aplicativo ou Revisão do Projeto.

### `projectId`

É a Identidade do Projeto em UUID v4 canônico, minúsculo e hifenizado. Ela é comparada como valor opaco e nunca é derivada de Nome, Localização ou conteúdo. A correção pré-sessão de uma Cópia externa é a única substituição técnica dessa Identidade permitida sem Salvamento criativo.

### `revision`

É a Revisão do Projeto representada pelo payload, codificada como inteiro no intervalo `0..=9_007_199_254_740_991`; a criação começa em `0`. Ela identifica estado criativo, não versão de esquema, versão do aplicativo, contagem de Salvamentos ou prova isolada de concorrência.

Durante uma Sessão, cada ação criativa concluída recebe um valor ainda não usado naquela Sessão; Undo e Redo podem restaurar uma revisão anterior. Depois de fechar, o Histórico deixa de existir e uma nova Sessão parte da revisão persistida. Ao atingir o máximo, novas ações criativas falham antes de mutar o estado, mas abrir, salvar o estado corrente e exportar permanecem permitidos.

`savedRevision` permanece transitória: ao abrir, recebe `revision`; após Salvar, só é atualizada quando `ProjectCore` consumir a prova privada de que o `ProjectStore` publicou os bytes daquela revisão. Migração de esquema e correção técnica de Identidade preservam a revisão.

### `project`

No schema v1, `project` é exatamente um `ProjectDocumentV1` com quatro campos obrigatórios: `document`, `visualDefaults`, `media` e `sheets`. Essa primeira versão cobre somente o primeiro fluxo persistente: criação, alteração de DPI, Undo/Redo, Salvamento, reabertura e Exportação JPEG da Lâmina visível. O editor demonstrativo da Fase 1 não transforma seus Frames e Fotos em compatibilidade pública.

### `document`

Possui exatamente:

| Campo | Tipo estrutural | Regra |
|---|---|---|
| `displayUnit` | `mm`, `cm` ou `in` | controla apresentação e entrada; não altera os valores físicos |
| `sheetWidthUm` | inteiro `1..=9_007_199_254_740_991` | largura física da Lâmina inteira; precisa ser par para que a Página tenha largura inteira em micrômetros |
| `sheetHeightUm` | inteiro `1..=9_007_199_254_740_991` | altura física compartilhada |
| `dpi` | inteiro `1..=1200` | Resolução do Projeto; coincide com o limite já aceito pelo domínio e pelo primeiro renderizador |
| `bleedUm` | inteiro `0..=9_007_199_254_740_991` | Sangria uniforme interna |
| `safetyUm` | inteiro `0..=9_007_199_254_740_991` | segurança uniforme medida após a linha de corte |

Todos os valores físicos são inteiros em micrômetros; `displayUnit` não autoriza duplicatas em ponto flutuante. O domínio rejeita combinações que eliminem a Área de corte ou a Área de segurança de qualquer Página ativa. Também rejeita uma combinação de dimensão e DPI cujo cálculo raster canônico do [primeiro fluxo JPEG](0014-contrato-jpeg-do-primeiro-fluxo.md) produza menos de `1` ou mais de `65.535` pixels em qualquer eixo; assim, toda geometria v1 válida permanece dentro do limite estrutural desse encoder. O guardrail provisório de memória da Exportação não é uma invariante do Projeto: uma geometria estruturalmente válida pode ser salva e reaberta mesmo quando uma tentativa JPEG desta versão retorna `ResourceLimitExceeded`.

O arquivo não define texto de entrada, separador decimal ou arredondamento da interface. A fronteira de criação entrega ao domínio micrômetros inteiros já convertidos; uma interface não pode aproximar silenciosamente uma entrada que não resulte em micrômetros inteiros. Trocar `displayUnit` preserva os inteiros autoritativos e altera somente sua apresentação. A política de edição e formatação desses campos pertence ao contrato da interface, não ao DTO persistente.

### `visualDefaults`

Possui exatamente `background`, `overlay` e `frameBorder`.

`background` é uma destas uniões fechadas:

```text
{ scope: "bothSides", both: BackgroundContentV1 }
{ scope: "perSide", left: BackgroundContentV1, right: BackgroundContentV1 }
```

`BackgroundContentV1` é `{ kind: "color", rgb: "#RRGGBB" }` ou `{ kind: "media", mediaId: UUID }`. Background nunca é nulo. A cor é opaca e usa exatamente seis dígitos hexadecimais maiúsculos; o valor neutro é `#FFFFFF`.

`overlay` é uma destas uniões fechadas:

```text
{ scope: "bothSides", both: OverlayContentV1 | null }
{ scope: "perSide", left: OverlayContentV1 | null, right: OverlayContentV1 | null }
```

`OverlayContentV1` é somente `{ kind: "media", mediaId: UUID }`; `null` representa ausência. Em `bothSides`, `left` e `right` são desconhecidos e proibidos; em `perSide`, `both` é desconhecido e proibido.

`frameBorder` é `{ kind: "none" }` ou `{ kind: "solid", rgb: "#RRGGBB", widthUm: n }`, onde `n` é inteiro em `1..=9_007_199_254_740_991`. No escopo inicial, a Borda é a única propriedade do Padrão de Frame do Projeto; Opacidade permanece uma propriedade local do Estilo do Frame e, como o v1 ainda não persiste Frames, não aparece no payload.

### `media`

É uma lista ordenada de zero ou mais objetos com exatamente:

```json
{
  "id": "00000000-0000-4000-8000-000000000003",
  "kind": "decorative",
  "path": {
    "encoding": "windowsUtf16",
    "units": [67, 58, 92, 70, 111, 116, 111, 115, 92, 97, 46, 112, 110, 103]
  }
}
```

O schema v1 aceita somente `kind: "decorative"`, pois apenas as escolhas provisórias de Background e Overlay entram no primeiro fluxo real. Cada `id` é UUID v4 canônico e único. Caminhos exatamente iguais não podem aparecer duas vezes nessa lista; aliases textuais diferentes continuam referências distintas conforme a política do Painel. Nome, dimensões, formato, orientação, datas, fingerprint, perfil de cor e paleta são derivados e não são persistidos.

Todo `mediaId` usado em `visualDefaults` precisa apontar para um item dessa lista. Mídia não referenciada continua válida no Painel; a criação inclui somente os Decorativos efetivamente confirmados.

### `sheets`

É uma lista ordenada de pelo menos duas Lâminas. Cada item possui exatamente `id`, como UUID v4 canônico e único, e `activeSides`, entre `both`, `left` e `right`.

A posição determina o Papel da Lâmina e a Numeração das Páginas; esses valores não são duplicados. A primeira Lâmina aceita `both` ou `right`, a última aceita `both` ou `left`, e toda Lâmina interna exige `both`. Nenhuma Lâmina pode ter zero lados ativos.

No schema v1, todas as Lâminas herdam `visualDefaults` e começam sem Frames. Ausência de mídia é um estado válido: o fixture neutro produz a Lâmina opaca com o Background branco e o plano de composição correspondente não exige fontes externas. Background e Overlay globais referenciados pelo v1 precisam participar tanto do Canvas quanto da [Exportação do primeiro fluxo](0014-contrato-jpeg-do-primeiro-fluxo.md); o que permanece adiado são aplicações locais, transformações desses Decorativos e a Pilha visual completa.

Aplicações locais, Frames, posicionamentos de Fotos, Layouts, favoritos e demais edições posteriores estão além do primeiro fluxo persistente e exigirão um schema público seguinte quando forem integrados a Projetos reais; não aparecem como campos vazios ou reservados no v1. A v2 amplia somente a categoria da lista de `MediaRef`, sem persistir posicionamentos.

Não pertencem ao payload:

- Nome ou Localização do Projeto, pois o Nome deriva do nome do arquivo e a Localização é o próprio caminho aberto;
- a última Localização autorizada e qualquer evidência física usada para reencontrar a Instância de arquivo do Projeto, pois esse estado pertence somente ao registro local da máquina;
- `savedRevision`, indicação de mudanças pendentes, Undo/Redo ou estado transitório da interface;
- Cache, representações reduzidas, metadados derivados ou disponibilidade observada das mídias;
- Recuperação de sessão, preferências globais, Logs ou resultados de Exportação;
- versão do aplicativo, checksum decorativo ou uma cópia do envelope.

Acrescentar um campo persistente depois da publicação exige uma nova `schemaVersion`; não se altera silenciosamente o DTO v1.

## Caminhos persistidos

Cada caminho de Arquivo vinculado usa exclusivamente o DTO nativo abaixo:

```json
{
  "encoding": "windowsUtf16",
  "units": [67, 58, 92, 70, 111, 116, 111, 115, 92, 97, 46, 106, 112, 103]
}
```

`units` contém unidades UTF-16 entre `0` e `65535`, preservadas sem normalização textual, inclusive quando não formam uma string Unicode válida. Não existe string paralela autoritativa. Depois do decode reversível, a política de caminhos valida se a forma absoluta é aceita para aquele propósito; o pathname continua separado da Identidade do Projeto e da identidade física do arquivo.

## Carregamento e validação

O pipeline de carga coordenado por `ProjectCore` usa `ProjectStore` nesta ordem:

1. lê um cabeçalho mínimo sem criar Sessão;
2. confirma `documentType` e identifica `schemaVersion`;
3. desserializa o DTO fechado daquela versão, rejeitando campos desconhecidos, ausentes, duplicados ou de tipo incorreto;
4. valida a forma primitiva de UUIDs, revisão, limites numéricos, discriminadores e cada DTO `windowsUtf16`;
5. aplica migrações puras e sequenciais até o DTO atual;
6. converte o DTO atual para tipos do domínio;
7. solicita ao `ProjectDomain` a validação de unicidade, integridade referencial e demais invariantes criativas;
8. devolve ao `ProjectCore` o valor validado e os metadados privados de persistência; somente o núcleo produz uma Sessão editável ou uma revisão imutável para leitura.

Falhas preservam o arquivo byte a byte, não criam Sessão e retornam resultado tipado. Uma versão pública futura recebe `UnsupportedFutureSchema`; uma versão antiga sem cadeia suportada recebe `UnsupportedLegacySchema`; conteúdo malformado ou que viola a forma fechada do DTO recebe `InvalidProjectDocument`; um pathname decodificado cuja forma não é aceita pela política recebe `InvalidPath`; uma invariante criativa inválida recebe `InvalidProjectState`. Essa classificação pertence à carga do documento; o contrato público de `ProjectCore` pode transportar contexto adicional sem colapsar uma categoria em outra.

## Evolução e migrações

Quando existir uma versão pública posterior, cada transformação cobre um único passo `ProjectDocumentVn -> ProjectDocumentVn+1`. Não existe migração genérica baseada em mapas JSON, salto direto que duplique regras nem um `v0` fictício para as fixtures dos spikes.

A abertura migra apenas em memória. Migração técnica não altera a Revisão do Projeto, não cria Histórico e não marca a Sessão como criativamente modificada. `ProjectCore` acompanha essa atualização técnica separadamente das mudanças criativas: fechar sem alterações deixa o arquivo antigo intacto; um `Salvar` explícito serializa o DTO atual na mesma revisão. A entrada somente leitura usada pelo lote também migra em memória e nunca regrava sua origem.

A correção pré-sessão da Identidade de uma Cópia externa não promove o schema e só existe na abertura editável e interativa. Para cada versão pública suportada, o `ProjectStore` conserva um reescritor estrito daquela mesma versão que altera somente `projectId`, preserva a revisão e publica o DTO `Vn` pela sequência atômica. Não existe patch textual de JSON. Se a versão não puder ser lida e reescrita integralmente, a cópia falha de forma fechada antes de montar estado local ou criar Sessão. A entrada headless e somente leitura nunca executa essa correção; quando encontra uma Cópia que exige nova Identidade, retorna `ExternalCopyRequiresInteractiveResolution` sem escrever na origem.

Como a barreira de exclusão é derivada da Identidade, a abertura editável mantém a barreira antiga enquanto adquire a barreira da nova Identidade. Ela só publica a substituição depois de obter ambas; depois da publicação, instala e verifica a nova trava física e o novo `PersistedBaseline` ainda sob as duas barreiras. A barreira antiga só é liberada após essa verificação. Cache, Recuperação e Sessão são montados exclusivamente com a nova Identidade e somente depois do processo inteiro, impedindo uma janela em que a origem e a Cópia usem a mesma Identidade ou em que a nova Identidade fique sem proteção.

### Política de exemplos versionados

Os exemplos acompanham versões públicas reais, e não versões inventadas para testar infraestrutura:

| Classe | Cobertura normativa atual |
|---|---|
| documento válido | o envelope neutro e a matriz abaixo permanecem exemplos v1; os contratos v2 e v3 conservam exemplos próprios |
| migração | a cadeia é `[v1 -> v2 -> v3]`, com entrada v1, arquivo dourado intermediário v2 preservado, resultado final v3, abertura sem escrita e promoção somente após `Salvar` |
| estados inválidos | a matriz abaixo preserva falhas v1; os contratos posteriores acrescentam seus próprios campos e falhas fechadas |

Os exemplos normativos estão em
`project_document_v1_migration_input.myalbuns`,
`project_document_v2_migration_expected.myalbuns` e
`project_document_v3_migration_expected.myalbuns`, sob
`crates/myalbuns-core/tests/fixtures/`. O arquivo dourado v2 conserva a etapa aceita
`v1 -> v2`; o arquivo dourado v3 prova separadamente o resultado atual após a cadeia
completa. Não se preenche a cadeia com `v0` nem com versões demonstrativas.

## Salvamento

O escritor emite somente a versão atual, com representação determinística para facilitar diagnóstico e fixtures, mas o leitor não depende da formatação. `ProjectCore` valida a revisão esperada e congela a candidata antes do I/O. O `ProjectStore` segue o contrato de temporário irmão, sincronização, publicação, nova trava e verificação exata já aprovado para o Salvamento atômico; só depois devolve um recibo privado de publicação para que `ProjectCore` confirme a Revisão salva na `ProjectSession`.

Cada arquivo aberto conserva no `ProjectStore` um `PersistedBaseline` privado formado pela trava e identidade física retidas e pelos bytes exatos validados na abertura. Sob a barreira de Salvamento, o Store exige que o destino continue sendo o mesmo objeto e contenha exatamente esses bytes; depois da publicação, exige que o novo handle contenha exatamente a candidata. Só então substitui o baseline. Revisões iguais com bytes diferentes continuam sendo conflito, portanto o número persistido nunca decide concorrência sozinho.

Falha, conflito ou resultado físico inconclusivo não confirma a revisão. Nenhuma API pública do domínio pode marcar uma revisão como salva sem a prova pertencente ao `ProjectStore` e ao Host do Projeto. O handshake, os resultados estruturados e os efeitos sobre a Sessão estão definidos em [Contrato público de persistência do ProjectCore](0015-contrato-publico-de-persistencia-do-project-core.md).

## Casos dourados obrigatórios

A implementação publica fixtures versionadas e testes para, no mínimo:

| Caso | Resultado |
|---|---|
| documento v1 completo, Identidade canônica, revisão válida e Projeto consistente | abre e preserva a Revisão salva |
| fixture neutra mostrada neste design, sem mídia | abre com duas Lâminas duplas, produz plano sem fontes externas e compõe Background branco opaco |
| padrão `perSide` referenciando dois Decorativos válidos | abre e preserva escopo, ordem das mídias e paths byte a byte |
| caminho local, UNC, mapeado, verbatim local/UNC e unidade UTF-16 não pareada | round-trip exato pelo DTO nativo |
| conteúdo válido com outra extensão, fornecido explicitamente por `Abrir Projeto` | abre normalmente |
| conteúdo válido com outra extensão, fornecido por `Projetos recentes` | abre normalmente |
| conteúdo válido com outra extensão, encontrado pela descoberta em lote | ignorado, pois não é candidato |
| conteúdo inválido com extensão `.myalbuns`, encontrado pela descoberta em lote | candidato reportado como inválido, sem escrita |
| abertura direta pelo Windows | somente `.myalbuns` é associado ao MyAlbuns |
| extensão correta com `documentType` incorreto | `InvalidDocumentType` |
| fixture demonstrativa com `schemaVersion: 3` incompatível com o DTO v3 fechado | `InvalidProjectDocument`, sem criar Sessão |
| versão pública futura | `UnsupportedFutureSchema`, sem escrita |
| versão antiga sem cadeia suportada | `UnsupportedLegacySchema`, sem escrita |
| campo desconhecido em uma versão conhecida | `InvalidProjectDocument`, sem perda silenciosa |
| `documentType` ou `schemaVersion` duplicado na raiz | `InvalidProjectDocument`, sem criar Sessão |
| qualquer campo duplicado dentro de `project` | `InvalidProjectDocument`, sem criar Sessão |
| UUID ausente, não canônico ou de versão incorreta | `InvalidProjectDocument` |
| revisão não inteira ou fora de `0..=9_007_199_254_740_991` | `InvalidProjectDocument` |
| dimensão não inteira ou fora do intervalo estrutural, ou DPI fora de `1..=1200` | `InvalidProjectDocument` |
| largura ímpar, eixo raster fora de `1..=65.535` ou recuos que eliminam uma área ativa | `InvalidProjectState` |
| união visual com campos do discriminador oposto, cor não canônica ou borda sólida sem largura positiva | `InvalidProjectDocument` |
| DTO de caminho com encoding desconhecido, campo extra ou unidade fora de `0..=65535` | `InvalidProjectDocument` |
| pathname decodificado cuja forma não é aceita para Arquivo vinculado | `InvalidPath` |
| `media.kind` diferente de `decorative` | `InvalidProjectDocument` |
| path duplicado ou padrão referenciando mídia ausente | `InvalidProjectState` |
| menos de duas Lâminas, ID repetido ou combinação de lados incompatível com sua posição | `InvalidProjectState` |
| Cópia externa em abertura editável | recebe nova Identidade no mesmo schema antes da Sessão, sob as duas barreiras |
| Cópia externa em entrada headless | `ExternalCopyRequiresInteractiveResolution`, sem escrita |

Com `schemaVersion: 3` atual, os casos dourados incluem o mesmo documento v1
seguido de `Salvar`, a abertura v1 sem escrita e a recusa de uma migração cujo
resultado viole qualquer DTO da cadeia ou as invariantes atuais. A correção de
Identidade de uma Cópia externa continua pertencendo ao ticket próprio e não é
absorvida pela migração. Não se cria uma versão antiga fictícia apenas para
exercitar a infraestrutura.

## Fronteira modular

`ProjectDocumentV1`, `ProjectDocumentV2`, `ProjectDocumentV3`, leitores,
escritores e migradores pertencem ao `ProjectStore`. `ProjectDomain` não deriva
`Serialize` ou `Deserialize` para satisfazer o arquivo e não conhece JSON,
extensão, esquema ou versões antigas. Mapeadores explícitos são a única
passagem entre DTO persistente e domínio atual.

As representações de IPC e frontend continuam contratos separados. Compartilhar nomes ou pequenos valores não autoriza reutilizar o envelope persistente como mensagem entre processos.
