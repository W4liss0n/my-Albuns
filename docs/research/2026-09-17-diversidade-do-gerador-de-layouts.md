---
status: current
document: research
date: 2026-09-17
---

# Revisão da variedade do Gerador de Layouts

A limitação relatada foi reproduzida. Aumentar somente o teto de dez para vinte
não resolve os casos com menos de cinco sugestões. A recomendação é ampliar
as composições para poucos Frames e selecionar em etapas, buscando de cinco
a dez opções e permitindo até vinte quando houver alternativas distintas e
com qualidade suficiente. Esta revisão não altera o algoritmo de produção.

## Constatações concretas

São limitações da cobertura e da política atuais diante da nova meta, sem
classificação P0–P3: o contrato aceito permite retornar menos de cinco ou
nenhum candidato. Não foi encontrada regressão contra esse contrato.

Foram medidas 165 consultas: quantidades de um a seis Frames; todas as
combinações de contagens V/H, com verticais antes das horizontais; e conjuntos
só de quadrados. Superfícies: Página de 300 × 300 mm e Lâminas de 600 × 300
e 600 × 240 mm. Nas Lâminas foram exercitadas ambas as permissões. Todas usam
margem de 15 mm, intervalo de 5 mm e menor lado de 20 mm.

Das 165 consultas, 73 retornaram menos de cinco opções, incluindo sete sem
candidatos. Todos os vinte casos de dois Frames ficaram abaixo de cinco.
Esses números descrevem a matriz testada, não a frequência em Projetos reais.
As medidas por consulta estão no [CSV](2026-09-17-diversidade-do-gerador-de-layouts.csv).

Exemplos com Lâmina de 600 × 300 mm e permissão por Página e por Lâmina:

| Frames | Atual | Somente teto de 20 | Teto de 20 e janela de nota ampliada |
| --- | ---: | ---: | ---: |
| 1 horizontal | 2 | 2 | 4 |
| 2 horizontais | 1 | 1 | 2 |
| 1 vertical + 1 horizontal | 2 | 2 | 4 |
| 3 horizontais | 4 | 4 | 7 |
| 2 verticais + 1 horizontal | 4 | 4 | 12 |
| 1 vertical + 2 horizontais | 10 | 12 | 14 |
| 2 verticais + 2 horizontais | 10 | 20 | 20 |

Na terceira coluna experimental de resultados, a janela mudou de dez para
vinte pontos abaixo da melhor nota, preservando o piso absoluto de 72. Os
resultados experimentais ainda não têm aprovação visual.

### 1. A busca oferece poucas composições para um ou dois Frames

- **Admission route:** Production-reachable.
- **Reachability or current consumers:** o Painel chama `queryLayouts`, que
  chega a `EditableProject::query_layouts_with_frame_request`, à sessão,
  a `LayoutRules::list` e a `generate_layouts`. Em um Projeto novo de
  600 × 300 mm, solicitar dois Frames horizontais na consulta pública da
  sessão retornou exatamente um candidato. Esse caminho corresponde ao
  seletor de quantidade do Painel em uma Lâmina vazia.
- **Evidence:** na mesma consulta, a enumeração produziu três candidatos;
  restaram duas geometrias válidas distintas e apenas uma passou pelo corte
  de nota. Em Página de 300 × 300 mm, um Frame horizontal, vertical ou
  quadrado recebeu uma opção. Quatro Frames todos horizontais ou todos
  verticais nessa Página não produziram candidatos nem antes da seleção final.
- **Current impact:** o usuário tem uma escolha muito restrita, mesmo em
  superfícies grandes. Reduzir filtros finais não cria composições ausentes.
- **Owning module:** `layouts/generator/families.rs`; `grids`, `local` e `pages`.
- **Recommended change:** ampliar as famílias pequenas com variações
  deliberadas de escala e proporção e repartições adicionais de destaque e
  apoio. O destaque já existe; deve ser ampliado, não duplicado. Para três
  a seis Frames, ampliar as repartições e a combinação dos blocos por Página.
- **Verification boundary:** matriz por quantidade, orientação, superfície e
  permissão em `generate_layouts`, reprodução pela consulta pública da sessão
  e comparação visual dos resultados no Painel real.

### 2. A seleção não tenta completar uma quantidade mínima

- **Admission route:** Production-reachable.
- **Reachability or current consumers:** a mesma consulta produtiva usa a
  seleção de `layouts/generator/mod.rs`. O frontend percorre os candidatos
  recebidos; não foi encontrado um corte adicional para dez na apresentação.
- **Evidence:** o algoritmo usa `max(72, melhorNota − 10)`, novidade mínima
  de 0,25 e cotas por família; encerra quando não há candidato que passe,
  mesmo com uma só opção. Subir apenas o teto para vinte manteve os 73 casos
  abaixo de cinco. Ampliar também a janela relativa para vinte pontos reduziu
  esse total a sessenta. Com 2 V + 1 H em 600 × 300 mm, havia 29 geometrias
  válidas distintas: nove passaram pelo corte atual e quatro foram escolhidas.
  A janela ampliada resultou em doze sugestões.
- **Current impact:** alternativas são descartadas mesmo quando a lista final
  fica muito curta. Algumas combinações já têm variedade para mais de dez;
  outras dependem primeiro de uma busca mais rica.
- **Owning module:** `layouts/generator/mod.rs`, classificação e seleção final.
- **Recommended change:** uma primeira seleção com o perfil principal; se
  ficar abaixo da meta, expansão de composições e uma segunda seleção com
  flexibilização limitada dos critérios estéticos. Estabelecer teto de vinte
  e meta usual de cinco a dez, sem completar com duplicatas. Os valores finais
  precisam de comparação visual; o experimento não aprova a janela de vinte.
- **Verification boundary:** contagem, diversidade, ordem determinística,
  qualidade visual e tempo da consulta e das prévias.

## Oportunidades arquiteturais

Não há necessidade demonstrada de reescrever o módulo ou deslocar a geração
para a interface. A separação atual já concentra a enumeração e a seleção em
um módulo puro. A melhoria cabe nele e em seus testes, preservando
`LayoutRules` como responsável por compatibilidade e prioridades.

Aumentar o teto também aumenta o número de prévias solicitadas pelo Painel,
que hoje prepara todas antes de apresentar a nova consulta. Medir esse custo
faz parte da implementação; não foi comprovado um problema de desempenho.

## Decisões anteriores e invariantes

O [ADR 0010](../adr/0010-gerar-layouts-por-composicoes-deterministicas.md)
e o [design 0026](../design/0026-contrato-do-gerador-e-da-aplicacao-de-layouts.md)
fixam até dez sugestões e rejeitam completar listas com variantes quase iguais.
A nova meta pede uma revisão explícita desse teto e da seleção, preservando a
intenção de variedade. O ticket [#28](https://github.com/W4liss0n/my-Albuns/issues/28)
registra a aprovação do perfil anterior. A consulta de issues com `wontfix`
retornou vazia; o único registro em `.out-of-scope` não trata de Layouts.

Devem permanecer: orientação dos Frames, margens, intervalo, menor lado,
ausência de sobreposição, centralização dos blocos por Página e permissão de
Travessia central. Grades repetitivas continuam excluídas pelo perfil aceito;
readmiti-las exigiria uma mudança estética específica. Espelhamentos já são
variações permitidas, mas não garantem por si só cinco alternativas.

O [ADR 0008](../adr/0008-garantir-layout-compativel-por-arranjo-de-reserva.md)
garante a organização automática por uma reserva que não aparece no Painel.
Ela não deve ser contada para atingir a meta de sugestões.

Para um único Frame, escala e proporção são as principais variações disponíveis
sob centralização obrigatória. Não se deve prometer cinco opções para toda
consulta sem avaliar sua qualidade e o espaço físico. A meta é tentar chegar
a cinco, não violar as restrições para atingir um número.

## Evidência pendente e validação

Os 39 testes existentes passaram: seis de geração, seis de regras e 27 de
sessão. O teste de corpus aceita de uma a dez sugestões, portanto não verifica
a nova meta. A implementação deve acrescentar casos aprovados com contagens
mínimas, preservação de qualidade e exceções por restrição física.

A medição usou o gerador real. Uma cópia temporária do seletor permitiu variar
os critérios; o resultado do perfil original dessa cópia foi comparado
integralmente com a produção em todas as 165 consultas. As sondagens
temporárias foram removidas do código do Projeto.

Não foram validadas a aparência das alternativas experimentais, todas as
permutações de ordem, misturas com quadrados, tamanhos menores, parâmetros
não padrão ou o custo de vinte prévias na interface. São fronteiras para a
próxima etapa, não defeitos comprovados.

Os comandos de validação foram conferidos na documentação oficial de
[Cargo](https://doc.rust-lang.org/cargo/commands/cargo-test.html) e
[GitHub CLI](https://cli.github.com/manual/gh_issue_list), após indisponibilidade
por cota do Context7. O ambiente usa Rust/Cargo 1.98 e GitHub CLI 2.96.
