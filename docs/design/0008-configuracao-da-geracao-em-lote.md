---
status: accepted
document: design
---

# Configuração da Geração de Projetos em lote

## Objetivo

A Geração de Projetos em lote é iniciada dentro da Janela do Projeto que servirá como modelo. Sua janela de configuração reúne origem, destino e análise antes de qualquer arquivo ser criado.

## Campos

A janela mostra:

- `Projeto modelo`, somente para consulta;
- `Pasta de origem`, contendo a árvore de pastas com Fotos;
- `Pasta de destino`, onde a hierarquia será recriada;
- quantidade de pastas que produzirão Projetos.

Origem e Destino aceitam os caminhos totalmente qualificados da [política de caminhos](0011-resolucao-e-politica-de-caminhos.md). Cada tentativa usa um `OperationPathContext` próprio para comparar raízes e identidades físicas resolvidas e impedir que duas representações do mesmo local escondam um Destino igual ou interno à origem. Antes de distribuir trabalho, congela seus bindings em um `RootBindingPlan`; uma nova tentativa cria outro contexto.

```text
┌──────────────────────────────────────────────────────────────────┐
│  Gerar Projetos em lote                                         │
├──────────────────────────────────────────────────────────────────┤
│  Projeto modelo    Álbum Formatura 2026                          │
│                                                                  │
│  Pasta de origem   [ caminho ]                       [ Escolher ] │
│  Pasta de destino  [ caminho ]                       [ Escolher ] │
│                                                                  │
│  42 Projetos serão gerados                                      │
├──────────────────────────────────────────────────────────────────┤
│                                  Cancelar   Verificar e gerar     │
└──────────────────────────────────────────────────────────────────┘
```

O Projeto modelo exibido é o estado visível da sessão, inclusive mudanças ainda não salvas. A geração não salva nem modifica esse Projeto.

## Verificação

`Verificar e gerar` executa a descoberta recursiva, valida origem e destino e identifica conflitos antes de criar ou sobrescrever qualquer arquivo.

A hierarquia relativa é calculada por componentes sob a raiz validada. Caminhos absolutos, `..` ou qualquer sufixo que escaparia do Destino são rejeitados.

Quando existirem pendências, a [Tela de Problemas](0005-tela-de-problemas.md) é aberta no contexto da geração. A gravação só começa após as decisões exigidas e uma confirmação explícita.

Cada Projeto de destino já existente aparece em uma linha própria. O usuário pode escolher `Sobrescrever` ou `Ignorar` por linha, ou usar `Sobrescrever todos` e `Ignorar todos`. Um Projeto aberto nunca entra em uma sobrescrita individual ou global: enquanto continuar aberto, `Sobrescrever` fica indisponível e o item só pode ser ignorado.

Depois que todos os conflitos recebem uma decisão, `Continuar Geração` é habilitado. A geração não começa automaticamente ao resolver a última linha.

Durante a execução, a operação usa o [Progresso de operação](0007-progresso-de-operacoes.md). Sucesso integral recebe confirmação curta; itens ignorados ou com falha são apresentados depois na Tela de Problemas.
