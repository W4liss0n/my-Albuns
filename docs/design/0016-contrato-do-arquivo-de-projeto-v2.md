---
status: accepted
document: design
date: 2026-08-11
updated: 2026-08-11
ticket: 44-programa-03a-representacao-reduzida-e-pausa-causal-do-cache
---

# Contrato do Arquivo de Projeto v2

## Objetivo

Publicar a primeira evolução real do arquivo `.myalbuns` para que uma
`MediaRef` persistente possa classificar Foto e Decorativo, sem reabrir o DTO
v1 nem antecipar Frames, movimentação ou promoção de Identidade. Este contrato
complementa o [contrato v1](0013-contrato-do-arquivo-de-projeto-v1.md) e cumpre
a política do [ADR 0009](../adr/0009-adotar-arquivo-myalbuns-json-versionado.md).

## Versão atual e delta fechado

`schemaVersion: 2` é a versão pública atual e possui DTO próprio e fechado.
Envelope, `projectId`, Revisão, documento físico, Padrões visuais, caminhos
`windowsUtf16` e Lâminas mantêm exatamente as formas e invariantes do v1. A
única ampliação é o discriminador de cada item de `project.media`:

```text
MediaRefV1.kind = "decorative"
MediaRefV2.kind = "photo" | "decorative"
```

Os campos continuam sendo somente `id`, `kind` e `path`. Nome, dimensões,
paleta, disponibilidade, identidade física, tamanho, datas, perfil, orientação,
fingerprint e representação reduzida são observações ou derivados e não entram
no Projeto. IDs permanecem UUID v4 canônicos e únicos; paths duplicados e
referências visuais quebradas continuam inválidos.

Aceitar Foto na lista não persiste Frames ou posicionamentos de Foto. A v2
permite que o Painel, `MediaResolver`, `MediaRuntime`, `MediaMonitor` e Cache
atravessem uma Foto real; a composição persistente continua limitada ao
contrato visual já publicado. Frames, aplicações locais, movimentação, Cópia
externa e `Salvar como` exigem seus próprios tickets e, quando alterarem o
payload, outra versão pública.

## DTOs e migração sequencial

`ProjectDocumentV1` e `ProjectDocumentV2` são tipos persistentes distintos,
ambos com rejeição de campos desconhecidos. O leitor identifica primeiro a
versão e desserializa somente o DTO correspondente. Não há um DTO compartilhado
que aceite ambos os números nem migração por mapa JSON.

A cadeia atual contém um único passo puro:

```text
ProjectDocumentV1 --migrate_v1_to_v2--> ProjectDocumentV2
```

O passo copia envelope, Identidade, Revisão, documento, Padrões, paths e
Lâminas sem alteração e converte cada Decorativo v1 no Decorativo v2
equivalente. Ele não inventa Fotos, não muda a Revisão, não cria Histórico e
não abre arquivos externos.

Após a migração, somente o DTO v2 é convertido para o modelo atual do domínio.
O escritor atual emite exclusivamente v2. Uma futura v3 deverá acrescentar um
novo DTO e o passo `v2 -> v3`; não poderá editar estes tipos retroativamente.

## Abertura e Salvamento

Abertura editável e abertura somente leitura migram v1 apenas em memória. O
Host conserva que a origem ainda exige atualização de schema separadamente do
estado criativo:

- fechar sem Salvar preserva os bytes v1 exatamente;
- abrir para lote ou leitura nunca regrava a origem;
- a Sessão migrada começa com a mesma Revisão salva e não fica criativamente
  alterada apenas pela migração;
- `Salvar` explícito publica v2 deterministicamente, na mesma Revisão quando
  não houve edição criativa;
- conflito, falha física ou resultado inconclusivo não confirma a promoção.

O protocolo de temporário irmão, sincronização, publicação, nova trava e
verificação continua pertencendo ao `ProjectStore`. Promoção de Identidade de
Cópia externa não faz parte do Programa 03A; até o ticket proprietário, a
autoridade inconclusiva falha fechada antes de montar Cache.

## Exemplos versionados normativos

O par de migração está no controle de versão:

- entrada:
  `crates/myalbuns-core/tests/fixtures/project_document_v1_migration_input.myalbuns`;
- resultado:
  `crates/myalbuns-core/tests/fixtures/project_document_v2_migration_expected.myalbuns`.

O resultado esperado preserva Identidade, Revisão, ordem, Padrões e unidades
UTF-16 e altera somente `schemaVersion`. Os testes públicos provam:

| Caso | Resultado |
|---|---|
| abrir a entrada v1 editável | modelo atual v2 em memória, mesma Revisão, origem intacta |
| abrir a entrada v1 somente leitura | leitura válida e zero escrita |
| fechar a Sessão migrada sem Salvar | bytes v1 idênticos |
| Salvar explicitamente a Sessão migrada sem edição | bytes iguais ao golden v2, mesma Revisão |
| abrir documento v2 fechado com Foto e Decorativo | ambos preservados como `MediaRef` persistente |
| `kind` desconhecido em v1 ou v2 | `InvalidProjectDocument`, sem Sessão |
| campo adicional em qualquer DTO | `InvalidProjectDocument`, sem perda silenciosa |
| versão maior que 2 | `UnsupportedFutureSchema`, sem escrita |

Fixtures v1 anteriores permanecem normativas para o leitor v1. O schema 3 dos
spikes continua não público e recusado; ele não participa da cadeia.

## Fronteira modular

DTOs, seleção da versão, migração e escrita pertencem ao `ProjectStore`.
`ProjectDomain` recebe somente o modelo atual validado e não conhece JSON,
versões antigas ou o marcador técnico de atualização. Contratos de IPC,
representações de Cache e tipos TypeScript continuam separados do arquivo.
