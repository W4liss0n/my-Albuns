---
status: current
document: research
date: 2026-09-13
ticket: 98
---

# Falta de espaço real na Exportação

## Decisão e alcance

O aviso depende de uma falha real de I/O na criação, gravação, finalização ou
publicação da Exportação. Não há consulta preventiva de espaço livre, estimativa
de tamanho, bloqueio por previsão ou limpeza automática para tentar liberar espaço.
Essa decisão complementa o tratamento do Cache entregue em #96/#97.

JPEG, PNG e PDF preservam a causa tipada até o Host. O diálogo de erro existente
orienta: “Não há espaço no destino para concluir a exportação. Libere espaço ou
escolha outra pasta e tente novamente.” Falhas de publicação também mantêm a
quantidade já publicada e a orientação sobre o conjunto parcialmente substituído.

## Evidência e implementação

O teste de publicação falhou inicialmente porque o fluxo real do Host convertia
o erro de I/O em “a preparação da Exportação está indisponível”. O teste dos
encoders também falhou inicialmente: a criação de JPEG retornava `encode_failed`.

A classificação compartilhada reconhece códigos nativos de disco cheio e as
categorias de I/O de espaço/cota esgotados, sem interpretar mensagens do sistema.
O caminho de publicação mantém essa causa; os encoders preservam `IoError`
explicitamente nas APIs de `image` 0.25.10 e `png` 0.18.1. Falhas ao esvaziar
buffers e sincronizar o arquivo também passam pela classificação.

O protocolo Host–Processador passa de 23 para 24, com `outputStorageFull` no
protocolo e `output_storage_full` no erro de Exportação entregue à interface.
Host e Processador precisam ser distribuídos juntos. Esta alteração não atualiza
as cópias de executáveis geradas anteriormente.

## Fronteiras verificadas

- Encoders reais JPEG/PNG/PDF: erro na criação, gravação parcial com imagens pequenas
  e maiores e sincronização final. Depois de
  remover a falha, a codificação conclui. Conflito com arquivo existente preserva
  seus bytes e não é tratado como disco cheio.
- Publicação pelo Host: erro antes do primeiro arquivo e depois de publicar um
  arquivo, com contagem correta. Saídas ainda não substituídas, originais e arquivos
  órfãos são preservados; uma nova tentativa conclui e aplica a limpeza normal.
- Preparação integral e parcial: falha tipada do Processador mantém a mensagem
  específica, descarta a preparação após confirmar o encerramento e preserva as
  saídas anteriores.
- Serialização e código de saída: a causa atravessa o protocolo tipado, incluindo
  o código de saída 31, sem ser convertida em imagem ausente ou falha genérica.

As falhas são injetadas em caminhos temporários exatos, nas fronteiras de I/O,
por suporte habilitado somente nos testes. Não foi preenchido um volume físico.
Os testes do fluxo do Host usam transporte controlado; os encoders usam arquivos
e codificação reais. Não houve mudança de layout nem nova aceitação visual nativa.

## Revisão

Revisões independentes do intervalo `9312b29...11be82f`:

- Standards: nenhum achado; classificação e mensagem têm um único dono, e a
  fronteira de gravação compartilhada atende aos três formatos existentes.
- Spec: nenhum achado; implementação e documentação atendem à #98 e à decisão
  de avisar somente após uma falha real.

## Validação final

Executada no Windows, com o código de `11be82f`:

- `npm test`: 1.157 testes aprovados.
- `scripts/Test-Rust.ps1`: 867 resultados aprovados, incluindo reabertura e
  Exportação com o Processador real, Importação real e os novos casos de disco
  cheio. As etapas seletivas executam explicitamente os testes de integração
  que a primeira passagem da suíte deixa marcados como ignorados.
- `scripts/Test-RustQuality.ps1`: formatação e Clippy aprovados, inclusive o
  supervisor de desenvolvimento.
- `npm run build`: contratos de domínio e IPC atualizados, TypeScript e
  compilação Vite aprovados.
- `git diff --check`: aprovado.

Os executáveis de teste anteriores permanecem na versão em que foram gerados;
esta entrega registra a implementação e a validação do código, sem gerar uma
nova distribuição do aplicativo.
