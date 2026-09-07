---
status: current
document: research
date: 2026-09-06
platform: windows
---

# Miniaturas na abertura e na primeira rolagem

Após corrigir a retenção das prévias já visitadas, restavam dois problemas:
a abertura podia deixar o Painel vazio até a primeira rolagem, e a primeira
passagem por outras fotos ainda iniciava Processadores para Cache existente.

## Reprodução

Foi aberta uma cópia isolada do Projeto com 134 Fotos e suas 134 representações
de Cache. A cópia recebeu outra Identidade e outro diretório de dados. O arquivo
do usuário e os originais foram preservados. A aplicação foi compilada com
Tauri em Debug, com o frontend de produção e janelas nativas normais; EdgeDriver
foi conectado ao WebView2 152.0.4191.66. As portas nativas não foram simuladas.

Na versão anterior, todas as 22 fotos visíveis permaneceram sem imagem durante
3,2 segundos de observação, iniciada 5,9 segundos após lançar o aplicativo.
Após saltar 600 pixels, somente 8 das 33 fotos visíveis tinham imagem ao fim
de 1,4 segundo. O painel inicial nunca recebeu uma demanda calculada por sua
geometria: dependia da notificação de interseção de uma Janela inicialmente
oculta. Os eventos de interseção também disparavam revisões separadas.

O teste `requests the initial measured viewport without waiting for a scroll
or observer paint`, em `MediaPanel.test.tsx`, falhou com demanda vazia antes
da correção. O teste nativo `reopening_hands_recovered_previews_to_first_demand_without_processing_originals`
falhou com zero resultados adotados, contra um esperado.

## Correção

- O Painel mede sua grade ao montar e mantém essa mesma medição na rolagem,
  redimensionamento e notificações de interseção. A abertura não depende da
  primeira pintura da Janela.
- A margem de pré-carga acompanha três alturas do viewport, com mínimo de
  122 pixels. A retenção recente continua pertencendo ao registro nativo e
  mantém seus limites de bytes, quantidade e estimativa de memória.
- A recuperação exclusiva do Cache já verifica e decodifica as representações
  indexadas. Agora conserva o SHA-256 dos bytes verificados para a primeira
  demanda, junto com a observação da origem que ainda corresponde ao vínculo.
  O Monitor adota a mesma evidência, evitando invalidá-la como se fosse uma
  mudança nova. O caminho de publicação existente confere novamente origem,
  índice e digest. Não há pixels de originais nesses resultados.

A adoção na reabertura segue a concessão já aceita no MVP para alterações
feitas com o programa fechado que preservem tamanho e datas. Processamento e
Exportação conservam seus contratos de validação. O detalhamento está no
[contrato do Cache](../design/0010-armazenamento-local-e-cache.md).

O React instalado é 19.2.8. A consulta pelo fluxo `find-docs` usou a referência
indexada mais próxima, 19.2.7, para medição do DOM após montagem e limpeza dos
efeitos; não houve migração de biblioteca.

## Resultado na aplicação nativa

| Cenário | Antes | Depois |
| --- | --- | --- |
| Abertura, sem mover o Painel | 0/22 após toda a observação | 21/22 na primeira amostra, 22/22 em seguida |
| Salto de 600 pixels | 8/33 após 1,4 s | 33/33 em aproximadamente 255 ms |
| Volta ao início | Ainda incompleto após 1,4 s no primeiro percurso | 22/22 na primeira amostra |

Uma segunda passagem usou eventos de roda do WebDriver na janela nativa, em
incrementos de 137 pixels. Todas as fotos de cada trecho estavam carregadas
na primeira amostra em que a rolagem de cada evento foi observada; as 134 Fotos
foram vistas. Voltar ao
início manteve as 22 imagens prontas. O percurso corrigido não iniciou nenhum
Processador de Imagens. As amostras verificam `naturalWidth` dos elementos
realmente visíveis, não prometem latência zero nem medem a pintura física da GPU.
Um salto além da margem de pré-carga ainda pode exigir a leitura das prévias.

Os registros reproduzíveis desta máquina ficam em
`D:\CodexBuilds\myalbuns-initial-previews`: `measure.mjs`, os resultados
`baseline-measure.json`, `fixed-measure.json` e `fixed-gradual-measure.json`,
as capturas `*-startup.png` e `*-final.png` e os logs. A comparação usou o mesmo
catálogo e Cache derivado. A primeira tentativa de lançar o executável corrigido
encerrou a Janela global antes do handshake do WebView; foi excluída das
medições. As execuções seguintes completaram os percursos.

## Verificação

Passaram 374 testes nativos do Host (15 casos dependentes de ambiente ignorados),
99 testes de interface e adaptador, compilação de contratos/TypeScript e
`clippy --workspace --all-targets -- -D warnings`. A regressão de publicação
exercita tanto o resultado do processamento quanto a reabertura, incluindo
origem alterada, artefato adulterado, índice inválido, retirada da Identidade,
demanda obsoleta, Monitor, remoção e Religação. Outros casos cobrem origem
ausente ou modificada antes da adoção. A medição nativa complementa esses testes
no fluxo efetivamente utilizado pelo aplicativo.
