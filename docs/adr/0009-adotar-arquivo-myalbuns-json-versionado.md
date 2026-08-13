---
status: accepted
date: 2026-08-03
updated: 2026-08-11
---

# Adotar `.myalbuns` como arquivo JSON versionado de Projeto

O Arquivo de Projeto será um único documento JSON UTF-8 com extensão `.myalbuns`, identificação interna de tipo e versão explícita de esquema. A extensão será associada ao MyAlbuns no Windows, de modo que o arquivo funcione como a entrada direta do Projeto, enquanto o formato interno permanece legível para diagnóstico, mas sem suporte a edição manual.

## Decisão

O primeiro contrato público começa em `schemaVersion: 1` e não promete compatibilidade com os schemas e extensões usados pelas fixtures dos spikes. O envelope identifica o tipo `myalbuns.project`, a Identidade do Projeto, a Revisão do Projeto confirmada e o payload persistente. Caminhos Windows usam exclusivamente o DTO reversível `windowsUtf16`; Nome, Localização, Histórico, estado transitório, Cache e Recuperação não são duplicados no conteúdo.

Cada versão possui DTO fechado e rejeita campos desconhecidos. Versões públicas antigas suportadas são migradas sequencialmente apenas em memória; o arquivo só recebe o esquema atual em um Salvamento explícito. Versões futuras ou inválidas são recusadas sem modificar o arquivo. `ProjectStore` possui JSON, detecção, migração e escrita, enquanto `ProjectDomain` recebe somente o modelo atual já validado.

Enquanto `schemaVersion: 1` foi a única versão pública, a cadeia de migração era `migrations = []` e não existia exemplo de migração legítimo. A cadeia vazia registrava a ausência de transformação; ela não era um exemplo. Não se inventa `v0`, não se promove o `schemaVersion: 3` dos spikes e não se cria uma transformação fictícia apenas para exercitar infraestrutura.

O [Contrato do Arquivo de Projeto v2](../design/0016-contrato-do-arquivo-de-projeto-v2.md) é a primeira evolução pública. Ele inclui no mesmo conjunto normativo a transformação tipada e sequencial `v1 -> v2`, a entrada v1 e o resultado v2 esperado, a prova de abertura sem escrita e a promoção para v2 somente após `Salvar` explícito.

## Alternativas consideradas

ZIP, SQLite e formatos binários foram rejeitados porque as mídias permanecem externas e o primeiro fluxo não precisa de múltiplos artefatos internos, consultas parciais ou escrita incremental. Promover o `schemaVersion: 3` dos spikes também foi rejeitado: ele não contém caminhos nativos nem DPI e inclui campos derivados que não pertencem ao contrato público.

## Consequências

- Acrescentar ou alterar campos persistidos exige uma nova Versão do esquema do Projeto e uma migração explícita quando houver compatibilidade.
- Uma nova versão pública não está completa sem os exemplos versionados válidos, inválidos e de migração correspondentes; a v2 conserva os exemplos v1 e acrescenta o par dourado `v1 -> v2`.
- A extensão ajuda o Windows a encaminhar o arquivo, mas a identificação interna continua obrigatória e autoritativa.
- O escritor pode produzir JSON determinístico e legível; leitores não dependem de espaços, quebras de linha ou ordem de propriedades.
- A implementação deve manter DTOs persistentes separados dos tipos de domínio e das representações de IPC.
