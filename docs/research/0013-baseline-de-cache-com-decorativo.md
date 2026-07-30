---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-30
updated: 2026-07-30
---

# Baseline de Cache com Decorativo transparente

## Pergunta

O Processador já produzia uma única representação reduzida por Foto e
preservava alfa ao receber um PNG. Faltava demonstrar que uma Imagem
decorativa real atravessa o mesmo contrato de Cache usado pelo Painel, pelo
Canvas e pela Grade, sem introduzir tiles, níveis progressivos ou previews
persistidos de Lâmina.

Esta rodada responde somente a esse gate. Aplicação interativa de Decorativos,
Background, escopo por lado, herança, arraste e Undo/Redo continuam nos tickets
de produto correspondentes.

## Contrato exercitado

O modelo deixou de representar Overlay por um booleano sem identidade:

- `MediaCatalogItem.kind` distingue `photo` e `decorative`;
- `SheetSnapshot.overlayMediaId` mantém a referência documental mínima;
- `CompositionCore` resolve essa referência para um `ComposedDecorative` com
  identidade, nome e retângulo de desenho;
- `EditorProjection` e `RenderSnapshot` recebem o mesmo `CompositionPlan`;
- o uso derivado da mídia conta Fotos em Frames e Decorativos em Overlay.

Não existe `hasOverlay` em paralelo, portanto não há dois estados capazes de
divergir. Background, herança e escopos não foram antecipados.

A interface continua recebendo um único mapa `mediaId → URL do Cache`.
`ProjectWorkspace`, `AlbumCanvas` e `SheetPreview` consultam esse mesmo mapa;
não foram criados mapas, DTOs ou caches específicos para Decorativos.

## Massa e instrumento

O corpus contém:

- 172 Fotos JPEG reais em dois Álbuns;
- um Decorativo PNG RGBA determinístico de 2400 × 1800 px, com alfa 96 no
  pixel central;
- 1.401,0 MiB de originais;
- SHA-256 agregado
  `c160ef3e3a2a1c7401f10574e3383fddb830678eb3fbc984dbca28dc59ebd01f`.

O runner recria o PNG fora das pastas de Fotos, inclui seus bytes no digest e
recalcula o corpus depois das duas alternativas. O Processador publica somente
um artefato de até 1600 px por mídia: JPEG quando opaca e PNG quando contém
transparência.

O gate rejeita a execução quando:

- um Projeto não publica exatamente um artefato PNG para seu Decorativo;
- o Canvas não materializa esse Decorativo como textura real do Cache;
- a identidade informada pelo Canvas não é `decorative-overlay`;
- a Exportação não abre também o original do Decorativo.

Testes pelas interfaces públicas verificam ainda que o card da aba
`Decorativos`, o Sprite PixiJS e o `<image>` da Grade recebem literalmente a
mesma URL do mapa compartilhado.

## Resultados

As duas alternativas foram executadas com dois Projetos e 100 Lâminas por
Projeto:

| Evidência agregada por alternativa | Resultado |
| --- | ---: |
| Fotos processadas | 172 |
| Decorativos processados | 2 |
| Artefatos gerados no Cache frio | 174 |
| Artefatos PNG do Decorativo | 2 |
| Canvas com textura decorativa real | 2 de 2 |
| Originais abertos pela Exportação da primeira Lâmina | 3 |

Cada Projeto publicou uma única representação PNG para o mesmo Decorativo. A
representação tem 1600 × 1200 px, preserva alfa 96 e não ganha um artefato JPEG
irmão. O original permanece inalterado.

A Exportação não usa o Cache. O Processador lê e verifica as duas Fotos JPEG e
o Decorativo PNG originais da primeira Lâmina, compõe o Overlay depois dos
Frames e preserva sua transparência no PNG final.

Os dados brutos, hardware, hashes e métricas das duas topologias estão no
[artefato JSON](artifacts/0005-long-album-navigation.json) e no
[resumo gerado](artifacts/0005-long-album-navigation.md).

## Falha encontrada pelo próprio gate

A primeira tentativa revelou que o runner repetia manualmente a versão 7 do
protocolo, enquanto o Processador já usava a versão 8. Um repro direto falhou
com código 27 usando 7 e passou usando 8.

O número duplicado foi removido. O Processador agora expõe
`--protocol-version`, o runner consulta essa interface antes de montar o
comando de reset e um teste de regressão fixa esse comportamento. A execução
completa posterior passou.

## Conclusão

O baseline de Cache está atendido:

- há uma representação reduzida por Foto ou Decorativo;
- conteúdo transparente permanece PNG e preserva alfa;
- Painel, Canvas e Grade reutilizam o mesmo contrato de URL;
- miniaturas de Lâmina continuam montadas em memória;
- Exportação permanece baseada nos originais;
- não foram introduzidos tiles, pirâmides, níveis progressivos ou previews
  persistidos de Lâmina.

Qualquer estrutura adicional precisa agora ser justificada por uma medição
posterior, e não por antecipação.
