---
status: accepted
date: 2026-09-26
---

# Abrir Projetos do myAlbuns antigo e convertê-los no primeiro Salvar

Os Projetos reais dos clientes foram criados no myAlbuns antigo, um aplicativo Python que grava um banco SQLite com a mesma extensão `.myalbuns`. O MyAlbuns abre esses arquivos diretamente: converte o conteúdo em memória, abre o editor sem aviso e só grava no primeiro `Salvar`, que avisa uma vez e substitui o arquivo antigo no mesmo caminho pelo [Arquivo de Projeto](../design/0051-contrato-do-arquivo-de-projeto.md) atual.

## Decisão

- **Reconhecimento.** Um arquivo cujos primeiros bytes são `SQLite format 3\0` é um Projeto do myAlbuns antigo. O `ProjectStore` o lê a partir dos bytes já carregados, sem abrir o SQLite no compartilhamento, e aceita os formatos `2.0`, `2.1` e `2.2` com `canonical_model_version = 2`. Os formatos `1.0` e `1.1` são recusados; o próprio myAlbuns antigo os migra ao salvar.
- **Conversão em memória.** O conversor escreve um documento `schemaVersion: 1` e o passa pelo leitor normal, de modo que o resultado tem exatamente a validação de um Projeto salvo. Travas e favoritos de Layout são restaurados pelo domínio. Identificadores de mídias, pastas e favoritos derivam do conteúdo, para que ler os mesmos bytes duas vezes produza o mesmo documento.
- **Abrir sem aviso.** A Sessão começa com alterações não salvas e nada é gravado. Fechar sem salvar mantém o arquivo antigo, que o myAlbuns antigo continua abrindo.
- **Primeiro `Salvar` com aviso único.** O `Salvar` pede confirmação antes de qualquer escrita: depois dele, o myAlbuns antigo não abre mais o arquivo. Confirmado, o Salvamento atômico existente substitui o arquivo no mesmo caminho; o Nome do Projeto não muda. O baseline é o conteúdo SQLite lido na abertura: se o myAlbuns antigo gravou o arquivo nesse intervalo, o Salvamento é recusado como conflito e nada é substituído. O diálogo de fechar de um Projeto antigo diz a mesma coisa, e `Salvar e fechar` converte sem outra pergunta.
- **Sem cópia do arquivo antigo.** Por decisão do autor em 26/09/2026.
- **`Salvar como`** grava o formato atual no destino, sem aviso, e deixa o arquivo antigo intacto.
- **Identidade.** Os UUIDs do myAlbuns antigo são v5 ou se repetem entre cópias de um mesmo modelo, e a Identidade nunca deriva do conteúdo. Cada Projeto antigo recebe um UUID v4 novo, guardado no registro local por conteúdo e local do arquivo até o primeiro Salvamento. Assim, reabrir o mesmo arquivo não salvo encontra a mesma Identidade, e a Recuperação de sessão continua válida depois de uma interrupção. O registro de Identidade e Localização só é publicado pelo primeiro Salvamento.
- **Perdas sem lista.** O que o programa atual não representa é descartado e registrado apenas no log de diagnóstico: filtros sépia e vintage ([#133](https://github.com/W4liss0n/my-Albuns/issues/133)), brilho, contraste e saturação ([#134](https://github.com/W4liss0n/my-Albuns/issues/134)) e zoom acima de 400% ([#135](https://github.com/W4liss0n/my-Albuns/issues/135)). Quando cada recurso existir, o conversor passa a mapeá-lo.
- **Bloqueios.** Um diário SQLite ativo ao lado do arquivo indica que o myAlbuns antigo está gravando, e a abertura é recusada. Uma lâmina do meio com uma só página ativa, ou um álbum com menos de duas lâminas, não cabe no Álbum atual e também é recusado.
- **Operações em lote.** A Exportação em lote e a Geração leem o Projeto antigo pela mesma conversão em memória e nunca gravam o `.myalbuns`.
- **Layouts personalizados.** No primeiro Salvamento, os Layouts criados pelo usuário na biblioteca do myAlbuns antigo (`%APPDATA%\MyAlbuns\layouts`, `author: "user"`) entram no catálogo global com a proporção daquele Projeto, porque um Layout só é oferecido em superfícies da mesma proporção. As bibliotecas que vinham com o programa antigo ficam de fora.

## Relação com o ADR 0009

O [ADR 0009](0009-adotar-arquivo-myalbuns-json-versionado.md) recusa importador para as versões de desenvolvimento do próprio MyAlbuns, que nunca chegaram a usuários. O myAlbuns antigo é outro programa, com Projetos reais em uso, e esta decisão não reabre aquelas versões. O SQLite continua rejeitado como formato: ele é apenas lido, em memória, e o arquivo gravado é sempre o JSON do contrato atual.

## Consequências

- O arquivo antigo só desaparece por uma escolha explícita do usuário no primeiro `Salvar`.
- Um Projeto antigo pode ser aberto, conferido e exportado sem ser convertido.
- A conversão depende de `rusqlite` com SQLite embutido no núcleo.
- A equivalência de enquadramento é verificada contra as fórmulas do myAlbuns antigo: a foto convertida mantém o centro e o tamanho exibidos em todas as combinações de Giro, Ângulo, Espelhamento, Zoom e Pan.
