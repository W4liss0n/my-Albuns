---
status: current
document: research
date: 2026-09-06
---

# Reuso da inspeção e do Cache na importação

A confirmação inicial do Monitor aproveita a inspeção concluída durante a
importação. A primeira demanda de prévias aproveita a geração de Cache já
verificada pelo Host. Nas sete rodadas com os mesmos cinco JPEGs, isso eliminou
as cinco chamadas adicionais ao Processador e reduziu a mediana das etapas
nativas medidas de **1.676,36 ms para 1.184,98 ms**, aproximadamente **29%**.

## Resultados

| Etapa | Antes: mediana | Depois: mediana |
| --- | ---: | ---: |
| Importação: inspeção e vínculo no Core | 199,04 ms | 217,94 ms |
| Primeira atualização das Fotos pelo Monitor | 336,54 ms | 0,65 ms |
| Geração e verificação do Cache | 842,70 ms | 868,74 ms |
| Primeira demanda, publicação e resposta das prévias | 297,35 ms | 66,28 ms |
| Total nativo medido por rodada, incluindo observações do Monitor | 1.676,36 ms | 1.184,98 ms |
| Chamadas extras ao Processador na primeira demanda | 5 | 0 |

O total é a mediana dos totais por rodada, não a soma das medianas da tabela.
A geração inicial continua executando cinco trabalhos, com até dois em
paralelo. A melhoria vem do reaproveitamento posterior; a geração do Cache
isoladamente não ficou mais rápida neste ensaio.

As cinco prévias conservadas nas duas medições têm os mesmos bytes, conferidos
por SHA-256.

O total variou entre 1.616,04 e 1.726,41 ms antes e entre 1.086,65 e 1.927,65 ms
depois. A primeira rodada posterior teve geração de Cache de 1.677,74 ms;
ela foi conservada nos resultados, sem atribuição de causa. As chamadas
adicionais foram zero e as cinco prévias foram entregues pelo novo caminho em
todas as sete rodadas. O resultado descreve esta máquina e esta amostra.

## Como funciona

`MediaResolver` observa a origem antes e depois de sua inspeção. `ProjectHost`
associa a evidência ao Projeto e à Foto importada. Na confirmação inicial do
Monitor, caminho, tipo, identidade física, tamanho e datas precisam coincidir
com a observação atual. A evidência é consumida nessa adoção. Divergência ou
ausência de comprovação conduz à inspeção normal. O Core continua sendo o
proprietário dos metadados da Foto.

`CacheEngine` conserva uma evidência pequena da preparação: observação da
origem, artefato publicado e SHA-256 dos mesmos bytes reduzidos decodificados
e validados pelo Host. As observações dessa preparação usam o `RootBindingPlan`
capturado pelo trabalho e compartilhado com o Processador, conforme o ADR 0007.
A demanda posterior resolve a origem atual e confere também a demanda, o índice
e o hash dos bytes que serão servidos.

A primeira demanda válida consome essa evidência e publica a prévia no registro
existente. Mudança da origem, corrupção, ausência do índice ou invalidação
encaminham o pedido ao processamento normal. Novos trabalhos, Religação,
retirada da Identidade e reconciliação do catálogo descartam evidências antigas.

As evidências guardam metadados e digest; não mantêm pixels do Original ou
prévias residentes fora da demanda. O índice persistido, a política de JPEG/PNG,
o limite de dois trabalhadores e a apresentação conjunta dos cartões mantêm
seus contratos. A passagem ocorre dentro da Sessão após uma geração verificada
pelo fingerprint integral. A decisão sobre alterações externas raras que
preservam os metadados continua sendo a do ADR 0001; a Exportação usa o Original.

## Método e limites

- Referência anterior: código em `d4c737b`; versão posterior: implementação
  desta mudança. Sete rodadas por versão, coletadas em momentos distintos.
- Windows, mesma máquina e mesmos cinco JPEGs reais: 25.037.650 bytes de
  originais. SHA-256 antes/depois confirmou que os originais não foram alterados.
- Cache novo a cada rodada, dois trabalhadores, mesmo Processador e perfil
  `dev` do projeto. O Cache de arquivos do Windows não foi esvaziado.
- Ensaio em cópia isolada do código, usando os módulos reais de importação,
  Monitor, `CacheEngine`, Processador nativo e registro HTTP de prévias.
- A medição anterior separava proposta do Resolver e vínculo no Core. A
  posterior chama `ProjectHost.import_photos`, incluindo a associação das
  evidências, para exercitar a composição real responsável pelo reuso.
- O transporte do ensaio inicia o executável nativo, mas exclui o handshake
  de inicialização e o bloqueio de escrita da integração Tauri. Também exclui
  agendamento concorrente do Monitor, diálogos, React e WebView. Portanto,
  o total não representa a duração completa percebida na interface.
- A instrumentação não mede pico de RAM. A medição anterior de decodificação
  no navegador não foi somada a este total nem repetida: o frontend não mudou.

Evidências locais: `.tools/import-flow-review/measurement/measurement.json`
e `.tools/import-flow-review/measurement-after/measurement.json`; estatísticas
comparadas em `measurement-after/comparison.json`. O ensaio anterior está em
`.tools/import-flow-review/measure-before.rs` e o posterior em
`.tools/import-flow-review/measure-after.rs`. Esses arquivos ficam fora do Git;
os caminhos privados das fotos e seus bytes não fazem parte desta documentação.

## Verificação

`npm run validate` concluiu suas seis etapas: compilação do Processador,
compilação e contrato do frontend, testes do frontend, testes de automação,
formatação/análise estática Rust e suíte Rust. A suíte inclui a Exportação pelo
Processador real. A cópia temporária usada na medição foi removida; os dados e
os dois arquivos de instrumentação foram conservados.

O teste de importação pelo Host seguido de confirmação do Monitor verifica a
ausência de nova decodificação e a inspeção normal quando a origem muda. O teste
de preparação seguida de demanda cobre origem alterada, corrupção dos bytes
reduzidos, índice inválido, demanda obsoleta, Monitor, remoção, Religação e
retirada da Identidade.

A revisão identificou uma divergência inicial no plano de raízes usado pelas
observações. O caso determinístico com `capture_with_binding` falhou antes do
ajuste e passou depois: a preparação conserva evidência da origem capturada e
uma demanda em outra raiz não consome essa geração. As revisões de padrões e
escopo foram concluídas sem apontamentos pendentes.

A unificação da validação com a geração em uma única decodificação do Original
continua como possível etapa futura. Exigiria preparar artefatos antes da
atribuição de Identidades de mídia pelo Core e uma decisão própria sobre essa
fronteira; não é necessária para os dois reaproveitamentos implementados aqui.
