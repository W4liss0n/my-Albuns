# 04 — Configurar e validar as Dimensões

**What to build:** tornar a etapa `Dimensões` completa e fazer cada configuração física válida chegar ao documento criado e ao editor sem uma segunda prévia gráfica.

**Blocked by:** 03 — Criar um Projeto padrão no local escolhido.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [ticket pai](../../programa-diagramacao/issues/08-esqueleto-ponta-a-ponta.md); [especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [criação de Projeto](../../../docs/design/0003-criacao-de-projeto.md); [Contrato do Arquivo de Projeto v1](../../../docs/design/0013-contrato-do-arquivo-de-projeto-v1.md).

- [ ] `Documento` contém Unidade (`mm`, `cm` ou `in`), largura e altura da Lâmina e DPI; `Estrutura` contém quantidade e formatos da primeira e da última Lâmina; `Áreas técnicas` contém Sangria e segurança.
- [ ] Largura e altura representam a Lâmina inteira, cada Página usa metade exata da largura, o Álbum contém ao menos duas Lâminas e os papéis ativos das extremidades seguem a ordem escolhida.
- [ ] Entradas aceitas são convertidas para micrômetros inteiros antes de sair da interface; a largura é positiva e par, a altura é positiva e DPI é inteiro em `1..=1200`.
- [ ] Um resumo somente de leitura atualiza imediatamente Lâmina, Página e DPI; a etapa não apresenta reprodução gráfica.
- [ ] Trocar Unidade converte os valores exibidos sem mudar o tamanho físico e preserva os equivalentes exatos de Sangria e segurança.
- [ ] Dimensão e DPI precisam produzir, pela fórmula canônica, eixos raster estruturais entre `1` e `65.535`; exceder apenas o guardrail transitório de memória não torna o documento estruturalmente inválido.
- [ ] `Próximo` só avança quando o conjunto inteiro é válido; cada erro aparece junto ao campo correspondente, o primeiro inválido recebe foco e nenhum modal genérico de validação é aberto.
- [ ] Corrigir um campo atualiza imediatamente seu erro e o resumo, sem perder os demais valores; voltar de `Personalização` conserva tudo o que foi preenchido.
- [ ] A criação bem-sucedida persiste as grandezas normalizadas, DPI, quantidade, lados ativos, Sangria e segurança no documento v1 e o editor apresenta a composição física correspondente.
- [ ] Sangria atua para dentro e segurança é adicional à linha de corte; combinações que eliminem a Área de corte ou de segurança em qualquer Página ativa são inválidas, sem margem na divisão central ou no lado inativo.
- [ ] Cancelamento ou falha de criação preserva o rascunho para nova tentativa e não publica parte de suas configurações.
- [ ] Testes de interface e fronteira pública cobrem valores padrão, Unidades diferentes, quantidade mínima, combinações de extremidades, zero independente para Sangria ou segurança, conversão sem mudança física, limites inválidos, foco e round-trip pelo arquivo.
