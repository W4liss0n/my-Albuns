---
status: accepted
document: design
date: 2026-09-17
---

# Validação de Pastas e elegibilidade de Layouts

Esta consolidação mantém os comportamentos dos designs
[0035](0035-pastas-de-organizacao-e-schema-v12.md) e
[0028](0028-layout-travado-e-schema-v9.md). As regras do documento pertencem ao
Core; o formulário e o Painel apresentam seus resultados.

## Nomes de Pastas

O módulo de Pastas oferece uma consulta sem mutação que recebe o tipo de mídia,
o nome digitado e, para renomear, a identidade da pasta atual. Devolve o nome
normalizado e um erro tipado: vazio, caracteres/comprimento inválidos, nome em
uso ou pasta inexistente. Consulta, edição e restauração usam a mesma política
de nomes. A consulta não altera revisão, Histórico, Salvamento nem Redo.

A UI mantém o rascunho e traduz o erro para a mensagem existente. Valida ao
enviar e, após uma tentativa, acompanha as correções do campo. As respostas são
associadas ao rascunho e à revisão do Projeto: respostas antigas não autorizam
envio nem substituem o erro de um rascunho novo. Uma consulta que falha impede
o envio e permite tentar novamente. Validação e envio pendentes impedem
submissões duplicadas.

O erro continua no campo, com descrição acessível e tooltip compartilhado,
sem deslocar controles. O comando de edição revalida o estado efetivo ao
executar; uma consulta válida não reserva o nome nem permite contornar a fila.

## Quantidade e aplicação de Layouts

Cada Lâmina projeta a faixa permitida de quantidades explícitas. A faixa é
ausente enquanto o Layout está travado ou quando a quantidade de Fotos excede
a cobertura. O seletor apresenta essa faixa e conserva a quantidade corrente
quando ela está fora da faixa. O Core usa a mesma política ao validar pedidos
explícitos; não mudou a consulta implícita de Lâminas acima da cobertura.

A consulta preparada também devolve, na ordem dos candidatos, quais exigem
cadeado. Esse fato vem dos placeholders reservados no patch, pela mesma regra
que protege o comando de aplicação simples. A UI usa o fato para desabilitar
o corpo da miniatura e apresentar sua orientação. Continua permitindo a prévia
e a confirmação pelo cadeado.

Quantidade solicitada, foco, hover e conservação temporária das miniaturas
permanecem na UI. Identidade do Projeto, revisão, alvo e identificador da
consulta continuam delimitando quais respostas e patches podem ser usados.
Os fatos projetados não são persistidos no arquivo e não alteram seu schema.

## Verificação

Os testes públicos do Core comparam a consulta de nomes com a edição real,
incluindo Unicode, limite de caracteres, nomes reservados, colisões por aba,
renomeação e alteração posterior à consulta. Os testes de Layout comparam a
faixa e os fatos dos patches com os comandos, incluindo redução, expansão,
travamento, cobertura e Undo/Redo.

Na UI, os testes verificam consumo dos resultados do Core, consultas atrasadas,
falhas, envio único, correção do rascunho e manutenção do tooltip. Os cenários
de Pastas e Layouts do manifesto de aceitação visual conferem a apresentação.
