---
status: current
document: research
date: 2026-09-05
updated: 2026-09-05
---

# Miniaturas de JPEG com metadados Adobe e sRGB do Windows

Após importar cinco Fotos, o Painel de Imagens permanecia em «Prévia indisponível».
A importação havia vinculado todos os arquivos; o Processador recusava os
Originais ao preparar as representações reduzidas.

## Causas e correção

O preflight exigia que o payload Adobe APP14 tivesse exatamente 12 bytes.
Os cinco Originais tinham um cabeçalho válido seguido de cinco bytes adicionais.
O marcador agora aceita pelo menos 12 bytes, preservando a recusa de cabeçalhos
incompletos, duplicados e transformações não suportadas. Isso corresponde ao
tratamento em [libjpeg-turbo 3.1.3, `examine_app14`](https://github.com/libjpeg-turbo/libjpeg-turbo/blob/3.1.3/src/jdmarker.c)
e ao parser do `zune-jpeg 0.5.15` usado por `image 0.25.10` no projeto.

Depois dessa correção, a reprodução revelou uma segunda recusa: o ICC sRGB
HP/Microsoft de 3.144 bytes não estava na allowlist. Seu conteúdo era idêntico
ao perfil padrão distribuído no Windows. A [Microsoft identifica esse arquivo
como perfil padrão para fontes sRGB](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/color-management-for-still-image-devices).
A allowlist passou a reconhecer seu tamanho e SHA-256 integral, fixados no
[contrato JPEG](../design/0014-contrato-jpeg-do-primeiro-fluxo.md).
O aplicativo não depende do perfil instalado nem aceita arquivos pelo nome.
A saída continua usando o sRGB2014 já incorporado.

## Evidência

- A regressão de APP14 falhou antes da primeira correção e passou depois dela.
- A regressão do perfil Windows falhou com `UnsupportedColorProfile` antes da
  segunda correção e passou depois, tanto em JPEG quanto em PNG. Uma alteração
  de um byte no perfil continua sendo recusada.
- O teste público do Processador
  `processor_builds_preview_for_adobe_jpeg_with_standard_windows_srgb` gera um
  JPEG sintético com os dois metadados, obtém uma prévia reduzida, verifica a
  reutilização do cache e compara os bytes do Original.
- As cinco Fotos do relato foram processadas pelo executável real em um cache
  isolado: cinco sucessos, prévias JPEG decodificáveis de 1600 × 1233 e todos os
  Originais inalterados. Duas prévias foram inspecionadas visualmente.
- Fotos, caminhos pessoais e dados do diagnóstico ficam apenas na pasta local
  ignorada `.tools/import-preview-diagnosis`; não fazem parte do repositório.

Comando focado: após inicializar a toolchain local, executar
`cargo test --package myalbuns-imaging --test cli processor_builds_preview_for_adobe_jpeg_with_standard_windows_srgb`.
A validação geral é `npm run validate`; seu relatório local identifica o commit
efetivamente testado. A janela nativa não foi aberta nesta verificação.

A lacuna era a cobertura de metadados de exportadores usados na prática.
As fixtures anteriores cobriam JPEG gerado pelo encoder do projeto e os três
perfis ICC já incorporados, mas não esta combinação de APP14 estendido e sRGB
legado. A regressão no comando público de cache cobre a combinação sem guardar
fotografias do usuário.
