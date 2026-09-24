---
status: accepted
document: design
date: 2026-09-24
---

# Contrato do Arquivo de Projeto

## Objetivo

Definir a primeira versão pública do Arquivo de Projeto (`.myalbuns`): envelope,
estrutura, validação, representação dos caminhos e regra de evolução. Este design
detalha o [ADR 0009](../adr/0009-adotar-arquivo-myalbuns-json-versionado.md) e
substitui os contratos de arquivo escritos durante o desenvolvimento
([0013](0013-contrato-do-arquivo-de-projeto-v1.md),
[0016](0016-contrato-do-arquivo-de-projeto-v2.md) e as seções de persistência dos
designs de recursos que criavam uma versão por recurso).

Até esta consolidação, cada recurso acrescentou uma versão de esquema e uma migração,
chegando a doze versões de desenvolvimento. Nenhuma delas chegou a usuários. Elas foram
descartadas: o formato abaixo recomeça em `schemaVersion: 1` e não possui migrações.

## Documento público

- a extensão é `.myalbuns`;
- o tipo apresentado pelo Windows é `Projeto MyAlbuns` e sua associação abre o arquivo diretamente no MyAlbuns;
- o conteúdo é um único JSON UTF-8, sem BOM;
- o formato é legível para diagnóstico, mas edição manual não constitui uso suportado;
- espaços, quebras de linha e ordem de propriedades não participam da compatibilidade;
- a extensão seleciona o aplicativo, mas não substitui a identificação interna do documento.

A extensão participa da associação do Windows e da descoberta automática, não da
validação autoritativa do conteúdo. Um caminho fornecido explicitamente por
`Abrir Projeto` ou `Projetos recentes` pode abrir com outra extensão quando
`documentType` e todo o documento forem válidos. A abertura direta pelo Windows associa
somente `.myalbuns`; a descoberta da Exportação em lote também considera somente
`.myalbuns` e reporta como inválido qualquer candidato dessa extensão cujo conteúdo não
seja um Projeto.

## Envelope

A raiz possui exatamente cinco campos:

```json
{
  "documentType": "myalbuns.project",
  "schemaVersion": 1,
  "projectId": "550e8400-e29b-41d4-a716-446655440000",
  "revision": 7,
  "project": { }
}
```

- `documentType` é sempre `myalbuns.project`. Ausência ou outro valor produz
  `InvalidDocumentType`, independentemente da extensão.
- `schemaVersion` é a versão do formato, não a versão do aplicativo nem a Revisão do
  Projeto. A única versão pública é `1`.
- `projectId` é a Identidade do Projeto em UUID v4 canônico, minúsculo e hifenizado. Ela
  nunca é derivada de Nome, Localização ou conteúdo. A correção pré-sessão de uma Cópia
  externa é a única substituição técnica permitida sem Salvamento criativo.
- `revision` é a Revisão do Projeto representada pelo conteúdo, inteiro em
  `0..=9_007_199_254_740_991`; a criação começa em `0`. Ela identifica estado criativo,
  não contagem de Salvamentos nem prova isolada de concorrência.

Durante uma Sessão, cada ação criativa concluída recebe um valor ainda não usado naquela
Sessão; Undo e Redo podem restaurar uma revisão anterior. Depois de fechar, o Histórico
deixa de existir e uma nova Sessão parte da revisão persistida. Ao atingir o máximo, novas
ações criativas falham antes de mutar o estado, mas abrir, salvar o estado corrente e
exportar continuam permitidos. `savedRevision` é transitória: ao abrir, recebe `revision`;
após Salvar, só é atualizada quando `ProjectCore` consumir a prova privada de que o
`ProjectStore` publicou os bytes daquela revisão.

## Estrutura de `project`

```json
{
  "album": {
    "displayUnit": "mm",
    "sheetWidthUm": 600000,
    "sheetHeightUm": 300000,
    "dpi": 300,
    "bleedUm": 3000,
    "safetyUm": 3000
  },
  "layoutSettings": {
    "permission": "pagesAndSheet",
    "marginUm": 15000,
    "gapUm": 5000,
    "minimumSideUm": 20000
  },
  "visualDefaults": {
    "background": { "sides": "both", "both": { "kind": "color", "rgb": "#FFFFFF" } },
    "overlay": { "sides": "both", "both": { "kind": "none" } },
    "frameBorder": { "kind": "none" }
  },
  "media": [
    { "id": "00000000-0000-4000-8000-000000000010", "kind": "photo", "path": "C:\\Fotos\\Noivos.jpg" }
  ],
  "mediaFolders": [
    {
      "id": "0e9d582d-e6c1-4c80-931d-f39f855e316e",
      "kind": "photo",
      "name": "Cerimônia",
      "mediaIds": ["00000000-0000-4000-8000-000000000010"]
    }
  ],
  "favoriteLayouts": [
    { "id": "9970c009-8b6e-4961-971c-fbb89c3f4679", "order": 0, "layout": { } }
  ],
  "sheets": [
    {
      "id": "00000000-0000-4000-8000-000000000001",
      "activeSides": "both",
      "layoutLocked": true,
      "lastLayout": { },
      "visuals": { },
      "frames": [ ]
    }
  ]
}
```

Todo o estado persistido do Projeto fica dentro de `project`; o visual de cada Lâmina
fica na própria Lâmina. `album`, `layoutSettings`, `visualDefaults` e `sheets` são
obrigatórios. Os demais campos são opcionais e seguem a regra de valores padrão abaixo.

### Convenções

- Toda medida física é um inteiro em micrômetros e termina em `Um`.
- Todo discriminador de variante chama-se `kind`, exceto a escolha entre conteúdo igual
  nos dois lados ou por lado, que usa `sides`.
- Todo identificador é UUID v4 canônico.
- Cores são opacas, no formato `#RRGGBB` com dígitos maiúsculos.
- Campos desconhecidos são recusados em todos os níveis.

### Valores padrão omitidos

O escritor omite todo campo opcional que contém o valor padrão, e o leitor trata a
ausência como esse valor. O leitor também aceita o valor padrão escrito por extenso; o
próximo Salvamento volta a omiti-lo.

| Campo | Padrão omitido |
|---|---|
| `media`, `mediaFolders`, `favoriteLayouts` | lista vazia |
| `mediaFolders[].mediaIds` | pasta sem mídia |
| `sheets[].layoutLocked` | `false` |
| `sheets[].lastLayout` | nenhum Layout aplicado |
| `sheets[].visuals` | a Lâmina herda Background e Overlay do Álbum |
| `sheets[].frames` | Lâmina sem Frames |
| `frames[].style` | o Frame segue o Padrão de Frame do Álbum |
| `frames[].photo` | Frame vazio |
| `photo.transform` e cada um de seus campos | enquadramento neutro |

### `album`

| Campo | Regra |
|---|---|
| `displayUnit` | `mm`, `cm` ou `in`; controla apresentação e entrada, sem alterar os valores físicos |
| `sheetWidthUm` | inteiro `1..=9_007_199_254_740_991`, par, largura da Lâmina inteira |
| `sheetHeightUm` | inteiro `1..=9_007_199_254_740_991` |
| `dpi` | inteiro `1..=1200` |
| `bleedUm` | inteiro `0..=9_007_199_254_740_991`, Sangria uniforme interna |
| `safetyUm` | inteiro `0..=9_007_199_254_740_991`, segurança medida após a linha de corte |

O domínio recusa combinações que eliminem a Área de corte ou a Área de segurança de uma
Página ativa, e dimensões cujo raster canônico fique fora de `1..=65.535` pixels em algum
eixo. O arquivo não define texto de entrada, separador decimal ou arredondamento da
interface.

### `layoutSettings`

`permission` é `pagesOnly` ou `pagesAndSheet`. `marginUm`, `gapUm` e `minimumSideUm`
são os parâmetros do gerador de Layouts, validados pelo domínio conforme o
[contrato do gerador](0026-contrato-do-gerador-e-da-aplicacao-de-layouts.md).

### `visualDefaults`

`background` e `overlay` usam a mesma forma por lado:

```text
{ sides: "both", both: C }
{ sides: "perSide", left: C, right: C }
```

No Background, `C` é `{ kind: "color", rgb }` ou `{ kind: "media", mediaId }`. No
Overlay, `C` é `{ kind: "none" }` ou `{ kind: "media", mediaId }`. `frameBorder` é
`{ kind: "none" }` ou `{ kind: "solid", rgb, widthUm }`, com largura positiva.

### `media`

Lista ordenada de Fotos e Decorativos do Projeto:

```json
{ "id": "…", "kind": "photo", "path": "C:\\Fotos\\Noivos.jpg" }
```

`kind` é `photo` ou `decorative`. Cada `id` é único e o mesmo `kind` não pode repetir um
caminho idêntico; uma Foto e um Decorativo podem apontar para o mesmo arquivo. Nome,
dimensões, formato, orientação, datas, fingerprint, perfil de cor e paleta são derivados e
não são persistidos. Background e Overlay só podem referenciar Decorativos; Frames só
podem referenciar Fotos.

### `mediaFolders`

Pastas de organização na ordem de criação, com `id`, `kind` (`photo` ou `decorative`),
`name` e `mediaIds`. Cada mídia pertence a no máximo uma pasta do seu tipo, e os nomes
seguem as regras do [design 0035](0035-pastas-de-organizacao-e-schema-v12.md). Uma
violação produz `InvalidProjectState`.

### `favoriteLayouts`

Cópias de Layouts favoritados no Projeto, cada uma com `id`, `order` único e `layout`.
A independência em relação ao catálogo global segue o
[design 0030](0030-favoritos-de-layouts-e-schema-v10.md).

Um Layout armazenado, usado aqui e em `sheets[].lastLayout`, tem a forma:

```json
{
  "origin": "automatic",
  "scope": "page",
  "surface": { "kind": "doubleSheet", "widthUm": 600000, "heightUm": 300000 },
  "positions": [ { "xUm": 19000, "yUm": 62667, "widthUm": 262000, "heightUm": 174666 } ]
}
```

`origin` é `automatic` ou `custom`; `scope` é `page` ou `sheet`; `surface.kind` é
`singlePage` ou `doubleSheet`. Posições nunca são negativas.

### `sheets`

Lista ordenada de pelo menos duas Lâminas. A posição determina o Papel da Lâmina e a
Numeração das Páginas. `activeSides` é `both`, `left` ou `right`: a primeira Lâmina aceita
`both` ou `right`, a última aceita `both` ou `left` e as internas exigem `both`.

- `layoutLocked` registra o Layout travado ([design 0028](0028-layout-travado-e-schema-v9.md)).
- `lastLayout` guarda o último Layout aplicado à Lâmina.
- `visuals` contém `background` e `overlay` próprios da Lâmina, cada um opcional:

```text
{ sides: "both", both: C }
{ sides: "perSide", left?: { content: C, mapping? }, right?: { content: C, mapping? } }
```

  Um campo ausente herda o valor do Álbum; em `perSide`, um lado ausente herda o lado
  correspondente do Álbum. `perSide` sem nenhum dos lados é recusado, pois não é um estado
  distinto de herdar. `mapping` é `side` (padrão) ou `bothSides`, quando o lado conserva
  sua metade de um conteúdo que cobria a Lâmina inteira antes da separação.

- `frames` lista os Frames em ordem de empilhamento:

```json
{
  "id": "…",
  "xUm": 19000, "yUm": 62667, "widthUm": 262000, "heightUm": 174666,
  "style": { "borderRgb": "#205070", "borderWidthUm": 2000, "opacityPercent": 65 },
  "photo": {
    "mediaId": "…",
    "transform": {
      "panX": 0.29, "panY": -0.48, "userZoom": 2.05,
      "quarterTurns": 3, "mirrorX": true, "angleTenths": 123, "blackAndWhite": true
    }
  }
}
```

  `style`, quando presente, é o Estilo do Frame completo, com `opacityPercent` em
  `0..=100`. Os valores neutros de `transform` são `panX: 0`, `panY: 0`, `userZoom: 1`,
  `quarterTurns: 0`, `mirrorX: false`, `angleTenths: 0` e `blackAndWhite: false`; seus
  limites seguem os designs de
  [composição com Foto](0017-contrato-da-primeira-composicao-com-foto.md),
  [orientação](0021-orientacao-de-fotos-e-projeto-v4.md),
  [ângulo](0022-angulo-fino-da-foto-e-projeto-v5.md) e
  [preto e branco](0023-preto-e-branco-da-foto-e-projeto-v6.md).

### Fora do arquivo

- Nome e Localização do Projeto: o Nome deriva do nome do arquivo e a Localização é o
  próprio caminho aberto;
- a última Localização autorizada e qualquer evidência física da Instância de arquivo,
  que pertencem ao registro local da máquina;
- `savedRevision`, mudanças pendentes, Undo/Redo e estado da interface;
- Cache, representações reduzidas, metadados derivados e disponibilidade das mídias;
- Recuperação de sessão, preferências, catálogo global de Layouts, Logs e resultados de
  Exportação;
- versão do aplicativo e checksums.

## Caminhos persistidos

Um caminho que forma texto UTF-16 válido é gravado como texto JSON comum:

```json
"path": "C:\\Fotos\\Cerimônia\\Noivos.jpg"
```

Somente um nome com unidades UTF-16 não pareadas, que não pode ser texto, conserva as
unidades exatas:

```json
"path": { "windowsUtf16": [67, 58, 92, 99, 97, 112, 97, 55296, 46, 112, 110, 103] }
```

Cada caminho tem uma única forma: unidades que formariam texto válido são recusadas como
`InvalidProjectDocument`. As duas formas são reversíveis e preservam o caminho sem
normalização, conforme o [ADR 0007](../adr/0007-tratar-caminhos-windows-e-identidade-fisica.md).
Depois da leitura, a política de caminhos valida se a forma absoluta é aceita; um caminho
recusado produz `InvalidPath`. Essa representação é exclusiva do arquivo; a IPC mantém o
seu próprio DTO de caminho.

## Carregamento e validação

`ProjectCore` usa o `ProjectStore` nesta ordem:

1. recusa BOM e lê um cabeçalho mínimo sem criar Sessão;
2. confirma `documentType` e aceita somente `schemaVersion: 1`;
3. desserializa o DTO fechado, recusando campos desconhecidos, ausentes, duplicados ou de
   tipo incorreto, e aplicando os valores padrão dos campos omitidos;
4. valida a forma de UUIDs, revisão, cores, limites numéricos, discriminadores e caminhos;
5. converte o DTO diretamente para o domínio, sem etapas intermediárias;
6. solicita ao `ProjectDomain` a validação de unicidade, referências, papéis das Lâminas,
   Layouts travados, favoritos e pastas;
7. devolve ao `ProjectCore` o valor validado e os metadados privados de persistência.

Falhas preservam o arquivo byte a byte, não criam Sessão e retornam resultado tipado:

| Situação | Resultado |
|---|---|
| `documentType` ausente ou diferente | `InvalidDocumentType` |
| `schemaVersion` maior que `1`, inclusive as numerações de desenvolvimento | `UnsupportedFutureSchema` |
| `schemaVersion: 0` | `UnsupportedLegacySchema` |
| JSON malformado, BOM ou violação da forma fechada | `InvalidProjectDocument` |
| caminho cuja forma não é aceita para Arquivo vinculado | `InvalidPath` |
| invariante criativa inválida | `InvalidProjectState` |

## Salvamento

O escritor emite sempre o mesmo texto para o mesmo estado: JSON indentado, UTF-8 sem BOM,
campos na ordem deste contrato, valores padrão omitidos e uma quebra de linha final.
Abrir e salvar sem editar reproduz o arquivo byte a byte. `ProjectCore` valida a revisão
esperada e congela a candidata antes do I/O. O `ProjectStore` segue o contrato de
temporário irmão, sincronização, publicação, nova trava e verificação exata do Salvamento
atômico; só depois devolve um recibo privado de publicação para que `ProjectCore` confirme
a Revisão salva na `ProjectSession`.

Cada arquivo aberto conserva no `ProjectStore` um `PersistedBaseline` privado formado pela
trava e identidade física retidas e pelos bytes exatos validados na abertura. Sob a
barreira de Salvamento, o Store exige que o destino continue sendo o mesmo objeto e
contenha exatamente esses bytes; depois da publicação, exige que o novo handle contenha
exatamente a candidata. Só então substitui o baseline. Revisões iguais com bytes
diferentes continuam sendo conflito. O handshake e os efeitos sobre a Sessão estão no
[Contrato público de persistência do ProjectCore](0015-contrato-publico-de-persistencia-do-project-core.md).

A correção pré-sessão da Identidade de uma Cópia externa lê o documento completo, troca
somente `projectId` e grava pela mesma sequência atômica; não existe patch textual de
JSON. Ela só existe na abertura editável e interativa. A entrada headless e somente
leitura retorna `ExternalCopyRequiresInteractiveResolution` sem escrever na origem.

## Evolução

A regra de evolução depois da primeira distribuição pública está no
[ADR 0009](../adr/0009-adotar-arquivo-myalbuns-json-versionado.md). Em resumo: toda
mudança no conteúdo persistido incrementa `schemaVersion`; um campo novo e opcional, cujo
valor padrão reproduz o comportamento anterior, não precisa de função de migração; somente
uma mudança incompatível recebe uma etapa de migração explícita, executada em memória e
gravada apenas em um `Salvar`.

## Exemplos e testes

Os exemplos normativos estão em
[`crates/myalbuns-core/tests/fixtures/project_file_v1/`](../../crates/myalbuns-core/tests/fixtures/project_file_v1/)
e são verificados por `crates/myalbuns-core/tests/project_file_v1.rs`:

| Exemplo | Conteúdo |
|---|---|
| `base.myalbuns` | Projeto sem Frames, com Background e Overlay usando um Decorativo |
| `photo.myalbuns` | Frame com Foto e enquadramento |
| `photo_angle.myalbuns` | Foto com ângulo fino |
| `photo_effect.myalbuns` | Foto em preto e branco |
| `folders_valid.myalbuns` | pastas de Fotos e Decorativos |
| `folders_invalid_duplicate_membership.myalbuns` | mídia em duas pastas, recusada como `InvalidProjectState` |
| `complete.myalbuns` | todos os recursos persistidos, incluindo caminho UNC, caminho com unidade não pareada, visuais por lado, estilo de Frame, todos os ajustes de Foto, Layout travado, favorito e pastas |

Todo exemplo válido é aberto e salvo por `Salvar como` e precisa reproduzir o arquivo byte
a byte. Um arquivo de desenvolvimento com `schemaVersion: 12` é recusado sem escrita.
`crates/myalbuns-core/tests/project_document_v1.rs` cobre as falhas da tabela de
carregamento, campos duplicados, BOM, UTF-8 inválido, formas de caminho e invariantes do
domínio.

## Fronteira modular

Os DTOs do arquivo, o leitor e o escritor pertencem ao `ProjectStore`
(`crates/myalbuns-core/src/project_store/project_file.rs`). Eles não reutilizam tipos do
domínio, de modo que refatorar o modelo não altera o arquivo por acidente.
`ProjectDomain` não deriva `Serialize` ou `Deserialize` para satisfazer o arquivo e não
conhece JSON, extensão ou esquema.

As representações de IPC e frontend continuam contratos separados. Compartilhar nomes ou
pequenos valores não autoriza reutilizar o envelope persistente como mensagem entre
processos.
