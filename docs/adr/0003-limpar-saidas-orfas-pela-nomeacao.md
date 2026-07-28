---
status: accepted
date: 2026-07-27
---

# Limpar saídas órfãs pela convenção de nomes

Quando uma nova Exportação JPEG ou PNG do Álbum inteiro produzir menos arquivos que uma exportação anterior, o programa identifica saídas órfãs pelo padrão exato `{nome-do-projeto}_{índice com três dígitos}` e pela extensão selecionada. Os modos por Lâmina e por Página compartilham esse namespace: a convenção não permite distinguir qual modo criou um arquivo anterior. A limpeza não utiliza manifesto ou arquivo auxiliar, só ocorre após o usuário confirmar a sobrescrita e depois que todas as novas saídas do Projeto forem publicadas com sucesso conforme o ADR 0006.

## Consequências

- Uma Exportação por intervalo nunca remove arquivos que não pertençam ao intervalo selecionado.
- Uma falha antes da conclusão preserva as saídas excedentes da exportação anterior.
- Uma Exportação integral confirmada substitui o conjunto anterior do mesmo Nome e extensão, inclusive quando ele foi criado no outro modo; a confirmação explica esse efeito.
- Na Exportação em lote, cada Projeto é concluído e limpo independentemente dos demais.
- Arquivos com outro Nome do Projeto ou outra extensão não são considerados, portanto podem permanecer após renomear o Projeto ou trocar o formato.
- Um arquivo criado manualmente que coincida com o nome, a extensão e um índice órfão será indistinguível de uma saída anterior e poderá ser removido após a confirmação explícita de sobrescrita.
- Nenhum arquivo de manifesto é criado no Destino da Exportação.
