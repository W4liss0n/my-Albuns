---
status: accepted
date: 2026-09-09
---

# Gerar Layouts por composições determinísticas

O autor aprovou a versão 9 do protótipo como base do Gerador, após comparar
quantidades, orientações, tamanhos e permissões de Travessia central. Adotamos
uma busca finita por composições alinhadas, seguida de classificação por
qualidade e diversidade. A consulta retorna até dez sugestões; não completa
a lista com variações quase iguais, grades repetitivas ou grupos incompletos.

O Gerador será um módulo puro dentro de `myalbuns-core`, com uma única
operação de consulta sobre valores imutáveis. A enumeração, os pesos e os
desempates pertencem a uma versão do algoritmo. Não dependem do relógio,
de serviços externos ou do conteúdo dos arquivos de Foto. Essa separação
permite trocar a busca sem mudar o documento, a prévia ou a aplicação.

`LayoutRules` mantém compatibilidade, identidade, prioridade entre origens,
Mapeamento e produção de `LayoutPatch`. `ProjectSession` continua sendo a
única proprietária da confirmação e do Histórico. A existência de uma
organização para as automações permanece garantida pelo arranjo de reserva
do [ADR 0008](0008-garantir-layout-compativel-por-arranjo-de-reserva.md), mesmo
quando a busca não encontra sugestões ou recebe uma quantidade fora de sua
cobertura. O Gerador não se torna proprietário dessa garantia.

Um catálogo exclusivamente manual exigiria manter combinações de quantidade,
formato e V/H. A enumeração irrestrita de retângulos gerou resultados sem o
alinhamento desejado no primeiro experimento. As famílias aprovadas limitam
a busca a estruturas reconhecíveis, aceitando uma variedade finita em troca
de resultados reproduzíveis e de uma base que pode ser refinada depois.

A definição do algoritmo foi antecipada em relação à sequência originalmente
prevista no ticket #28, por decisão do autor. A validação visual do protótipo
não substitui a validação da aplicação real do ticket #25. O
[contrato do Gerador e da aplicação](../design/0026-contrato-do-gerador-e-da-aplicacao-de-layouts.md)
separa essas entregas e preserva as evidências da versão aprovada.
