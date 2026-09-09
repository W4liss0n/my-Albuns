# Referências do contrato de Layouts

`generator-v1.json` acompanha o
[contrato de geração e aplicação](../../../../../docs/design/0026-contrato-do-gerador-e-da-aplicacao-de-layouts.md).
O arquivo é um corpus de valores imutáveis para a implementação futura;
não é um catálogo de produção nem o código do Gerador.

Os 18 exemplos preservam 34 geometrias da V9 aprovada, convertidas para
micrômetros. Cada posição corresponde ao perfil de mesmo índice na consulta.
Os exemplos são amostras, não a lista completa nem uma promessa de manter
os mesmos índices de classificação em qualquer versão posterior.

Os quatro exemplos negativos registram grade incompleta, blocos por Página
descentralizados, Travessia central proibida e orientação incompatível.
`expectedViolations` identifica os erros que uma implementação deve detectar;
outros erros adicionais podem coexistir no mesmo exemplo. Os cinco casos
de consulta registram saídas sem sugestões e entradas inválidas esperadas.

`reference` identifica o commit, a versão e o modo de apresentação usados
para obter as geometrias. Agrupar espelhados foi uma opção da demonstração,
não um campo obrigatório da consulta de produção. O JSON não contém Fotos,
identificadores reais de Projeto ou vínculos com arquivos do usuário.

A conferência desta entrega verifica contagem, V/H/quadrado, margens,
tamanho mínimo, separação, escopo, centralização e ausência de células
vazias. A tolerância numérica é de 1 µm por borda. A aplicação real ainda
deverá atravessar `ProjectCore` para provar prévia, confirmação, Histórico,
Salvamento e Exportação; validar este corpus não substitui esses testes.
