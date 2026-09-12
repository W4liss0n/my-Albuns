---
status: current
document: research
date: 2026-09-12
---

# Exportação normal: integração e verificação

A Exportação do Projeto aberto reúne escopo, modo, formato e destino no mesmo
diálogo, conforme o design [0004](../design/0004-exportacao-normal.md).
O botão Exportar e o menu Arquivo abrem o álbum inteiro; Exportar Lâmina abre
um intervalo preenchido com a lâmina de origem. O usuário pode alterar as opções
antes de iniciar. Qualidade JPEG começa em 100 em cada abertura; dois cliques
restauram 100. PNG e PDF não apresentam esse controle.

A seleção usa a composição visível congelada e não salva implicitamente o
Projeto. Índices pertencem ao álbum completo, inclusive quando apenas um
intervalo é exportado. Páginas de abertura e fechamento contam somente seus
lados ativos. Cada página é rasterizada em suas próprias dimensões físicas,
preservando o mapeamento de fotos e fundos que atravessam o centro da lâmina.

O processador recebe uma tentativa com todas as saídas e mantém abertos os
Originais necessários até concluir a preparação. No Windows, esses handles
impedem escrita e exclusão enquanto a tentativa os utiliza. A composição é
reutilizada pelos três codificadores: JPEG com qualidade selecionada, PNG RGB8
com perfil e densidade, e um PDF com imagem RGB8 sem perdas por página, perfil
sRGB2014 e caixas físicas. O protocolo foi atualizado de 22 para 23.

Todos os arquivos são preparados e verificados antes da primeira publicação.
Conflitos exigem confirmação explícita do conjunto. A publicação é atômica por
arquivo; uma falha informa que o destino pode conter saídas anteriores e novas.
Somente o álbum inteiro confirmado para sobrescrita remove saídas excedentes
com o nome numerado canônico e a extensão atual, depois de publicar todas as
novas saídas. Intervalos preservam arquivos externos à seleção. Arquivos de
outro formato e nomes parecidos não são removidos.

A proteção de Originais usa o catálogo inteiro, incluindo fotos de outras
lâminas e mídias ainda não colocadas. Antes de substituir ou limpar um alvo
existente, compara sua identidade física com esses Originais. Uma comparação
inconclusiva bloqueia a alteração e orienta usar uma pasta nova ou restabelecer
o acesso; exportar para nomes novos não exige essa varredura adicional.

As ações de religar arquivos continuam na pasta diretamente escolhida pelo
usuário. Ao resolver o último bloqueio de mídia, a mesma seleção, formato e
destino são retomados automaticamente. O fluxo antigo de exportação de uma
lâmina permanece como fronteira de compatibilidade dos ensaios nativos; a
interface produtiva utiliza o comando de Exportação normal.

## Evidências

- Teste real do processador: uma lâmina com Original de duas cores gera duas
  páginas em JPEG, PNG e PDF, com dimensões calculadas individualmente; o
  Original permanece idêntico em bytes.
- Leitura independente do PDF com pypdf 6.10.0, em modo estrito: duas páginas,
  caixas de 36 × 36 pontos, imagens de 13 × 13 pixels, ausência de rotação,
  perfil sRGB2014 e pixels incorporados idênticos aos PNG correspondentes.
- Testes do pipeline verificam preparação completa antes da publicação,
  preservação dos arquivos anteriores quando a codificação falha e limpeza
  das preparações. A política de saídas excedentes é exercitada nos escopos
  integral e parcial, em sucesso e falha.
- Cenários visuais declarados em `src/test/uiAcceptanceScenarios.json` cobrem
  JPEG, PNG por página, PDF, intervalo inválido, entrada contextual com uma
  lâmina, verificação inicial e confirmação de conflitos.

As evidências geradas ficam fora do Git em `.scratch/normal-export-rendering/`
e `.scratch/ui-acceptance/normal-export-final/`. Os resultados finais dos
ensaios e da revisão acompanham a entrega; capturas não equivalem à aprovação
manual do usuário.

## Dependências consultadas

Foram usadas as versões fixadas no projeto: image 0.25.10, png 0.18.1 e flate2
1.1.9. A API do PNG foi conferida na [documentação oficial da versão](https://docs.rs/png/0.18.1/png/struct.Encoder.html)
e os codificadores existentes e fontes locais fixadas fundamentaram as demais
chamadas. O contexto de consulta externo estava sem cota; não houve atualização
de versões para implementar esta tela.

## Roteiro manual

1. Exportar um álbum inteiro em JPEG para uma pasta nova e conferir a contagem.
2. Exportar uma lâmina dupla por página em PNG: conferir os dois lados e seus
   índices originais, incluindo uma foto que atravesse o centro.
3. Exportar o mesmo intervalo em PDF e conferir páginas, enquadramentos e tamanho.
4. Repetir uma exportação no mesmo destino, cancelar a sobrescrita e depois
   confirmar. Conferir que um intervalo preserva os outros arquivos da pasta.
5. Religá-lo após mover um Original e verificar a continuação automática com
   as opções escolhidas. Cancelar uma tentativa durante a preparação.

A Exportação em lote continua pertencendo ao seu programa próprio (#39).
Esta entrega não declara concluídas todas as fronteiras avançadas de
recuperação, negociação de capacidades e observações persistentes previstas
no contrato 0019.
