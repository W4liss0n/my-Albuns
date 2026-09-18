---
status: accepted
document: design
date: 2026-09-18
implementation-readiness: ready-for-agent
---

# Controles visuais compartilhados entre álbum e lâmina

O painel de Design da lâmina usa as mesmas linhas compactas de Fundo e
Sobreposição do Design do álbum. Esta decisão foi solicitada pelo usuário em
18/09/2026, tomando como referência os controles originais do álbum. A proposta
gerada por imagem, com seta e botão escrito “Imagem”, foi rejeitada.

## Apresentação e propriedade

`VisualDesignControl` é o proprietário compartilhado da linha, do rótulo, das
amostras de 26 px, da opção sem sobreposição e da composição com o seletor de
decorativos. Seu CSS tem nomes neutros. Álbum e lâmina fornecem seus valores e
ações; o componente não executa mutações nem determina o escopo.

O álbum mantém o rascunho e o botão Aplicar. A lâmina mantém o seletor de cor com
confirmação e tooltip de validação. O seletor de decorativos usa o catálogo já
importado, tanto no álbum quanto na lâmina. Na lâmina ele envia o comando
existente de aplicação de decorativo para o lado escolhido.

## Estado e ações da lâmina

- As duas linhas substituem a lista de amostras, códigos de cor e frases de
  origem exibida anteriormente. Os detalhes de cada lado continuam no título
  do rótulo e na descrição acessível do grupo.
- Cores diferentes são representadas na amostra dividida. Um escopo com
  sobreposições diferentes não aparece selecionado como “Sem sobreposição”.
- Remover fundo fica em um botão compacto com ícone e nome acessível; a amostra
  sem sobreposição executa a remoção da sobreposição. Usar padrão do álbum fica
  ao lado, com ícone de restauração, quando existe personalização local.
- Selecionar um lado não altera o projeto. A mudança de lâmina ou de lado fecha
  os seletores e descarta a cor ainda não aplicada.
- Uma operação em andamento conserva seu alvo e bloqueia novas alterações dos
  dois controles até concluir, inclusive se a seleção do lado mudar.
- Ao fechar um seletor por clique em outro controle, o foco acompanha o controle
  escolhido. Escape ou clique em uma área sem controle devolvem o foco ao botão
  do seletor. Abrir a cor após o seletor de imagem não fecha o editor de cor.

As regras de herança, remoção, histórico e salvamento continuam nas operações
existentes. A lâmina de página única mantém apenas seu lado ativo. Esta decisão
prevalece sobre a apresentação anterior dessas linhas na referência visual.

## Verificação

Testes de componente cobrem cor, cancelamento, validação, restauração, seletores
e operações pendentes. Testes do workspace verificam o comando e o escopo
enviados pelo seletor de decorativos. A aceitação visual usa os cenários do
manifesto do projeto, incluindo álbum, lâmina, cores distintas, origem mista,
página única e seletores abertos.
