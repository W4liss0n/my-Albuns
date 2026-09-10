---
status: accepted
document: design
date: 2026-09-10
ticket: 24
platform: windows
implementation-readiness: ready-for-agent
---

# Painel de imagens

Este contrato complementa a especificação do Painel e os contratos de
[estado](0012-propriedade-de-estado-e-modulos-do-nucleo.md) e
[importação](0020-importacao-com-decode-unico-e-lotes.md).

## Catálogo, preferências e ausência

O Painel mantém seleção, âncora e busca por aba durante a Janela aberta.
A seleção solicitada após importar é um evento consumido uma vez; alterações
de ordenação e filtro não o repetem. Ocultar o Painel conserva esse estado;
trocar de Projeto o reinicia. Aba, chave e direção da ordenação, filtro de uso
e tamanho das miniaturas pertencem ao armazenamento global de preferências.

O Host fornece criação, alteração e disponibilidade a partir das observações
estabilizadas do Monitor, correlacionadas com o vínculo atual. Essa consulta
não acessa Originais nem depende da demanda de miniaturas. Ausentes ficam
depois dos demais arquivos em qualquer direção de ordenação; datas conhecidas
precedem desconhecidas e o Nome natural desempata.

O aviso de ausentes abre uma revisão temporária. Durante essa revisão, busca,
filtros de uso e aba ficam sobrepostos às escolhas anteriores. Encerrá-la
retoma as escolhas preservadas, sem gravar a revisão como preferência.

## Importação e gestos

Fotos e Decorativos usam a mesma tentativa nativa. A aba é capturada ao iniciar
a ação e define o tipo de todos os novos vínculos. O Core distingue duplicatas
por tipo e caminho: o mesmo Original pode ter um vínculo em cada aba.
Arquivos, pasta e soltura convergem antes do processamento e do único commit.
Uma pasta fornece imagens diretamente contidas nela, sem percorrer subpastas.
Arquivos explicitamente selecionados passam pela inspeção mesmo quando a
extensão não corresponde ao conteúdo; o formato real decide a aceitação.

JPEG, PNG e TIFF atravessam o processamento já usado pelas Fotos. O raster
decodificado fornece dimensões e prévia, conservando transparência. Inspeção
alternativa, falhas parciais, adoção no Monitor e publicação do Cache mantêm
seus donos. A associação de uma prévia ao vínculo definitivo usa tipo e caminho,
para não confundir Foto e Decorativo do mesmo Original.

A soltura do Windows é recebida pelo WebView e convertida de coordenadas
físicas para pixels CSS antes do teste de pertencimento à área do Painel.
O gesto interno usa eventos de ponteiro e o limiar de arraste do Windows.
Assim, mover uma miniatura dentro do programa coexiste com a recepção de
arquivos externos. Solturas de Fotos consultam o alvo atual no Core;
respostas de movimentos anteriores e gestos cancelados não fazem commit.

## Remoção da seleção

O Painel é dono do alvo de `Delete` e do menu `Remover`. Campos de texto
conservam seu tratamento de teclado; a seleção e a âncora continuam transitórias.
A confirmação aguarda comandos já pendentes antes de contar os usos e bloqueia
novas interações enquanto estiver aberta. A decisão referencia uma única revisão
criativa; outra revisão exige revisar a seleção novamente.

O Core recebe a seleção inteira em uma ação. Remover tudo elimina Frames
destravados que usavam as Fotos; posições travadas são esvaziadas. Manter Frames
esvazia todas as ocorrências e preserva a estrutura. Itens sem uso saem junto
com o restante da seleção. Os Originais permanecem intactos e Undo restaura o
catálogo e a composição juntos.

## Contratos externos

Versões verificadas: React 19.2.8, API JavaScript do Tauri 2.11.1,
Tauri Rust 2.11.5, diálogo 2.7.2 e `image` 0.25.10. `find-docs` foi usado
antes das decisões; a consulta indexada atingiu a cota. A confirmação usou
documentação oficial e as interfaces da versão instalada.

A [configuração do Tauri](https://v2.tauri.app/reference/config/#dragdropenabled)
documenta a substituição do arraste HTML5 pelo manipulador nativo no Windows.
[onDragDropEvent](https://v2.tauri.app/reference/javascript/api/namespacewebview/#ondragdropevent)
define posições físicas e a função para cancelar a assinatura. O código
instalado de `tauri-plugin-dialog` confirma `pick_files` e `pick_folder`
assíncronos e o resultado opcional de cancelamento.
O processamento reutiliza os decodificadores de PNG e TIFF já presentes no
programa e as APIs verificadas no contrato de importação.
