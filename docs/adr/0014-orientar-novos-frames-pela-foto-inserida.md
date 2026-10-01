---
status: accepted
date: 2026-09-17
updated: 2026-09-30
---

# Orientar novos Frames pela Foto inserida

O autor relatou e confirmou em 17/09/2026 que inserir uma imagem vertical criava
um Frame horizontal. A criação reutilizava sempre o perfil manual de 3:2, mesmo
com as dimensões corretas da Foto disponíveis na sessão. A correção ficou numa
branch que não foi integrada e foi retomada em 30/09/2026, com a revisão das
sugestões do [ADR 0013](0013-diversificar-e-harmonizar-as-sugestoes-do-gerador.md).

Ao criar um Frame a partir de uma Foto, a orientação inicial passa a acompanhar
as dimensões observadas da imagem, já corrigidas pela orientação EXIF: vertical
usa 2:3, horizontal usa 3:2 e quadrada usa 1:1. Isso vale para duplo clique e
arraste em área vazia, no modo normal e no Modo de edição. O tamanho continua
proporcional à superfície ativa e limitado por sua altura. Não se exige conservar
a proporção exata do arquivo; o Gerador pode variá-la dentro da mesma orientação.

A Criação manual de Frame placeholder continua usando 3:2. Preencher ou
substituir uma Foto em um Frame existente preserva sua geometria e estilo.
Quando não há dimensões observadas para o vínculo atual, permanece o perfil
manual de 3:2; não se inventa uma orientação a partir da projeção provisória
de 1 × 1 pixel. Observar metadados posteriormente não altera Frames existentes.

Na escolha automática, Último Layout, Favoritos e Personalizados só têm
prioridade quando conservam as orientações dos Frames atuais, inclusive o
recém-criado. Continuam disponíveis no Painel para aplicação explícita,
recuperando sua geometria original. A reserva para consultas sem composição
adequada mantém o contrato do [ADR 0008](0008-garantir-layout-compativel-por-arranjo-de-reserva.md).

A correção reutiliza as dimensões já observadas pelo Core; não adiciona
metadados ao documento, schema, comando IPC ou parâmetros no Gerador.
Criação, reorganização e seleção continuam sendo uma única ação de Histórico.

Esta decisão atualiza a regra inicial do
[design 0027](../design/0027-integracao-dos-layouts-e-schema-v8.md) e distingue
a escolha automática da aplicação explícita no
[design 0026](../design/0026-contrato-do-gerador-e-da-aplicacao-de-layouts.md).
