---
status: current
document: research
date: 2026-09-26
platform: windows-11-x64
---

# Prova ponta a ponta dos Projetos do myAlbuns antigo

Verifica, com Projetos reais de clientes, a decisão do
[ADR 0012](../adr/0012-abrir-projetos-do-myalbuns-antigo.md): abrir o `.myalbuns` do
myAlbuns antigo, convertido em memória, e só substituí-lo no primeiro `Salvar`.

## Exportação comparada com o programa antigo

**Método.** Cada arquivo foi lido diretamente no compartilhamento, só para leitura, pelo
teste ignorado `real_processor_exports_old_myalbuns_projects_without_writing_them`
(`src-tauri/src/batch_runner/tests.rs`). O teste percorre o mesmo caminho da
Exportação em lote: leitura, conversão, observação das fotos, plano e processador real.
Ele exporta em JPEG as lâminas sem quadros vazios e confere que o arquivo antigo não
mudou. A referência foi renderizada pelo `ExportLaminaRenderer` do próprio myAlbuns
antigo, sem nitidez. As lâminas de uma só página foram comparadas com o lado
correspondente da referência.

**Resultado.** Diferença absoluta média por pixel, de 0 a 255:

| Projeto | Lâminas | Diferença média |
|---|---|---|
| Modelo RR Formaturas ATUALIZADO | 15 | 0,35; 1,3 a 1,7 nas lâminas com sobreposição própria |
| AUGUSTO SERAFIM-2 (Gutto) | 18 | 1,0 a 6,8 |
| JOSE CARLOS-7 (Gutto) | 16 | 1,2 a 5,5 |

Os dois Projetos do Gutto cobrem fundo em imagem por página, bordas personalizadas,
Ângulo, capa e contracapa de uma página, Zoom e Pan. A diferença restante está só nos
contornos, efeito de reamostragem.

**Defeito encontrado.** Na primeira rodada, as fotos saíram esticadas (média em torno de
50). A composição somente leitura (`freeze_rendering`) tratava como 1 × 1 toda Foto não
observada, e o processador desenha a Foto no retângulo composto. A Exportação em lote
passou a ler o cabeçalho de cada Foto, com as dimensões já orientadas pelo EXIF, e a
compor com `freeze_rendering_with_photo_sources`. O defeito afetava a Exportação em lote
de qualquer Projeto com fotos. A prévia da tela de boas-vindas já informava as
dimensões.

**Diferença conhecida.** O programa antigo desta máquina exportava com nitidez
`moderate`. O MyAlbuns não aplica nitidez, então as lâminas saem um pouco mais suaves
que as entregues pelo programa antigo.

**Disponibilidade.** Os Projetos já entregues da RR e da StudioLab perderam as fotos,
porque as pastas `04 - Separado` foram limpas. Por isso foram usados os Projetos do Gutto
(`GUT004`), que ainda têm todas as fotos.

## Jornada na janela real

**Método.** Build debug do aplicativo, com dados isolados
(`MYALBUNS_PROCESS_GATE_DATA_ROOT`) e uma cópia do Modelo RR ao lado dos seus decorativos.
As janelas foram conduzidas pelo DevTools do WebView2.

**Resultado:**

- ao abrir, o editor mostra "Alterações não salvas", com a conversão pendente, e o arquivo
  antigo não muda;
- o painel mostra a lâmina com 49,2 × 30,6 cm, a página com 24,6 cm e a borda com
  0,28 cm, como no programa antigo;
- depois de uma edição, o ponto de Recuperação é gravado; encerrar o aplicativo à força
  não altera o arquivo antigo;
- ao reabrir, aparece "Recuperar trabalho não salvo?". Em "Recuperar e abrir", o Projeto
  volta com a mesma Identidade, a edição e a conversão ainda pendente;
- no primeiro Ctrl+S aparece "Salvar no formato novo?". Cancelar não grava. Salvar grava o
  formato atual no mesmo caminho, apaga o ponto de Recuperação e importa o Layout
  personalizado da biblioteca antiga;
- ao reabrir, o Projeto está limpo, e Ctrl+S não mostra aviso nem regrava o arquivo;
- em outra cópia, fechar sem salvar mostra no diálogo que o programa antigo deixará de
  abrir o arquivo, e "Salvar e fechar" converte.

**Observação.** Fechar um Projeto antes de terminar a abertura mostra "Não foi possível
abrir o projeto" ([#137](https://github.com/W4liss0n/my-Albuns/issues/137)).

## Ferramentas

Os roteiros de referência (`render_old.py`, que usa o ambiente Python do programa
antigo), de comparação (`compare.py`) e da jornada (`ui-journey.mjs`) ficaram fora do
repositório, em `.scratch/legacy-export-proof-20260926/` nesta máquina. Eles dependem dos
arquivos de clientes no compartilhamento e do código do programa antigo.
