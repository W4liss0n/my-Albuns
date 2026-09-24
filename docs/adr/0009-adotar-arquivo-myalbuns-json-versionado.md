---
status: accepted
date: 2026-08-03
updated: 2026-09-24
---

# Adotar `.myalbuns` como arquivo JSON versionado de Projeto

O Arquivo de Projeto será um único documento JSON UTF-8 com extensão `.myalbuns`, identificação interna de tipo e versão explícita de esquema. A extensão será associada ao MyAlbuns no Windows, de modo que o arquivo funcione como a entrada direta do Projeto, enquanto o formato interno permanece legível para diagnóstico, mas sem suporte a edição manual.

## Decisão

O envelope identifica o tipo `myalbuns.project`, a versão do esquema, a Identidade do Projeto, a Revisão do Projeto confirmada e o conteúdo persistente. Nome, Localização, Histórico, estado transitório, Cache e Recuperação não são duplicados no conteúdo. O DTO do arquivo é fechado, recusa campos desconhecidos e é separado dos tipos de domínio e das representações de IPC. `ProjectStore` possui JSON, detecção de versão e escrita; `ProjectDomain` recebe somente o modelo já validado.

A primeira versão pública é `schemaVersion: 1`, definida no [Contrato do Arquivo de Projeto](../design/0051-contrato-do-arquivo-de-projeto.md). Durante o desenvolvimento, cada recurso criou uma versão e uma migração, chegando a doze versões. Nenhuma chegou a usuários, então todas foram descartadas antes da publicação: a estrutura foi reorganizada e recomeçou em `1`, sem migrações. Arquivos dessas versões, assim como os `.myalbum` e o `schemaVersion: 3` dos spikes, não recebem importador.

### Evolução depois da primeira distribuição

Até a primeira distribuição pública, a v1 ainda pode ser ajustada no lugar, desde que contrato, exemplos e testes mudem juntos. A partir dela:

- toda mudança no conteúdo persistido incrementa `schemaVersion`, para que uma instalação antiga recuse o arquivo novo como versão futura em vez de perder campos;
- um campo novo e opcional, cujo valor padrão reproduz exatamente o comportamento anterior, não precisa de função de migração: o leitor aceita a versão anterior com o mesmo DTO, e o valor padrão é omitido na escrita;
- uma mudança incompatível, como renomear, mover ou mudar o significado de um campo, recebe uma etapa de migração pura e tipada da versão anterior para a seguinte, com exemplo de entrada e resultado esperado;
- a abertura migra apenas em memória, sem alterar a Revisão do Projeto nem criar Histórico; o arquivo só recebe a versão atual em um `Salvar` explícito;
- versões futuras ou inválidas são recusadas sem modificar o arquivo.

Não se cria versão fictícia nem migração apenas para exercitar infraestrutura.

## Alternativas consideradas

SQLite foi reavaliado na consolidação e rejeitado:

- Projetos podem estar em caminho UNC ou unidade mapeada ([ADR 0007](0007-tratar-caminhos-windows-e-identidade-fisica.md)), e o SQLite não garante travas confiáveis em sistemas de arquivos de rede;
- os arquivos auxiliares `-wal` e `-shm` quebram o arquivo único, a detecção de Cópia externa ([ADR 0002](0002-identificar-copias-externas.md)) e o `Salvar como`;
- o Projeto segue o modelo de documento com Salvamento explícito, sem consulta parcial nem escrita incremental que justifiquem um banco;
- o arquivo deixaria de ser legível para diagnóstico.

O tamanho do JSON não é o gargalo. Medido em 2026-09-24, um Projeto com 172 Fotos e 30 Lâminas ocupa cerca de 190 KB no formato v1 indentado (417 KB no formato de desenvolvimento, que gravava cada caminho como uma lista de unidades UTF-16) e é interpretado em cerca de 1,4 ms; um álbum de 800 Fotos fica perto de 1 MB. O tempo de abertura está nas prévias e no Cache.

ZIP e formatos binários, como MessagePack ou CBOR, foram rejeitados: as mídias permanecem externas, não há múltiplos artefatos internos, e o ganho de alguns milissegundos não compensa a perda de diagnóstico.

## Consequências

- Uma nova versão pública não está completa sem exemplos válidos e inválidos e, quando houver migração, o par de entrada e resultado esperado.
- A extensão ajuda o Windows a encaminhar o arquivo, mas a identificação interna continua obrigatória e autoritativa.
- O escritor produz JSON determinístico e legível, e abrir e salvar sem editar reproduz o arquivo; leitores não dependem de espaços, quebras de linha ou ordem de propriedades.
- Caminhos são gravados como texto quando formam UTF-16 válido e como unidades exatas apenas quando não formam, preservando a reversibilidade exigida pelo [ADR 0007](0007-tratar-caminhos-windows-e-identidade-fisica.md).
