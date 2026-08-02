---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-08-02
updated: 2026-08-02
---

# Topologias de processos em editores consolidados

## Pergunta e resposta curta

Esta pesquisa investiga como aplicativos desktop criativos e profissionais
maduros organizam documentos, Janelas, processos de interface, GPU, tarefas
pesadas, extensões e Recuperação, e o que esses precedentes significam para as
duas alternativas do MyAlbuns:

- **A — host independente por Projeto**: um `MyAlbuns.Project.exe`, uma Janela
  e uma `ProjectSession` por Projeto;
- **B — host multiwindow**: um host mantém várias Janelas e
  `ProjectSession`s.

Não há base pública para afirmar que A ou B seja “o padrão da indústria”. Nos
casos selecionados que publicam detalhes internos, a regularidade mais
consistente é **coordenação compartilhada com isolamento seletivo** de
renderização, extensões ou trabalho pesado conforme o risco e o ciclo de vida.
Essas arquiteturas são híbridas, não uma correspondência simples entre
documento, Janela e processo.

Para o MVP do MyAlbuns, **A é compatível com essa família híbrida**:
`MyAlbuns.exe` e exclusividades globais permanecem compartilhados, enquanto cada
Projeto recebe seu próprio host. Os precedentes não determinam essa
granularidade. A recomendação combina os requisitos de falha e Recuperação da
Sessão do Projeto com as medições próprias do spike.

## Metodologia e limites

A consulta foi encerrada em 2 de agosto de 2026. Foram aceitas somente fontes
primárias: documentação e suporte oficiais, textos de engenharia publicados
pelos responsáveis e código-fonte oficial. Relatos de usuários, listas de
processos de terceiros e artigos comparativos foram descartados.

Os casos foram escolhidos por comparabilidade ou transparência arquitetural,
não por amostragem de mercado. Portanto:

- a seleção não mede participação de mercado nem permite calcular uma
  “maioria”;
- uma interface com abas ou várias Janelas comprova um fluxo
  multidocumento, mas **não** comprova quantos processos possuem o estado;
- um renderer por Janela não comprova que o estado criativo do documento
  pertence a esse renderer;
- quando a fonte oficial não publica a fronteira de processos, o caso é
  marcado como não classificável em A/B;
- detalhes versionados ou históricos não são generalizados para versões ou
  mecanismos de extensão diferentes.

O texto usa três níveis de afirmação:

- **fato documentado**: declarado diretamente pela fonte;
- **observação**: padrão que aparece em mais de uma fonte;
- **inferência para o MyAlbuns**: consequência arquitetural proposta aqui,
  sujeita às evidências do próprio produto.

## Documento, Janela, processo e estado não são a mesma coisa

Quatro decisões diferentes aparecem misturadas com frequência:

1. **modelo de produto** — quantos documentos ou Projetos podem ficar abertos;
2. **modelo de Janelas** — abas, Janelas destacáveis ou uma Janela por Projeto;
3. **modelo de processos** — quais superfícies e serviços compartilham um
   processo do sistema operacional;
4. **propriedade do estado** — onde vive a autoridade mutável de cada
   documento e qual unidade pode ser salva ou recuperada independentemente.

Electron, por exemplo, documenta um processo principal por aplicativo e um
renderer para cada `BrowserWindow`, mas isso não determina onde um produto
concreto mantém seu estado de domínio. O Chromium pode ainda reutilizar
renderers quando as restrições permitem. Consequentemente, classificar um
editor apenas por sua aparência leva a conclusões falsas.

## Casos com evidência arquitetural útil

| Sistema | Fatos documentados | Leitura para A/B |
| --- | --- | --- |
| **Chromium** | Há coordenação central e múltiplos renderers. O isolamento limita o impacto de crash ou travamento, melhora paralelismo e segurança, com sobrecarga de memória por processo. A alocação pode reutilizar processos conforme compatibilidade e recursos disponíveis. [Modelo atual de processos e Site Isolation](https://chromium.googlesource.com/chromium/src.git/+/refs/heads/main/docs/process_model_and_site_isolation.md) | Híbrido adaptativo. A fronteira publicada é uma instância de site, não uma Janela nem uma `ProjectSession`; o valor do caso está nos critérios de isolamento e reuso, não em copiar sua granularidade. |
| **Electron** | Cada aplicativo possui um único processo `main`; cada `BrowserWindow` cria um renderer. `UtilityProcess` é indicado para serviços não confiáveis, intensivos de CPU ou propensos a falhar. [Process Model](https://www.electronjs.org/docs/latest/tutorial/process-model). Alguns `webContents` podem compartilhar renderer, e derrubar esse renderer pode afetar outros conteúdos. [webContents](https://www.electronjs.org/docs/latest/api/web-contents) | Coordenador compartilhado com isolamento por superfície e por tarefa. É B no shell e semelhante a A em algumas fronteiras, sem provar propriedade de estado por Janela. |
| **Visual Studio Code** | A engenharia oficial descreve um `main` global, renderer por Janela e extension host ligado à Janela; o isolamento evita que extensões prejudiquem operações da interface. [Migração para sandbox](https://code.visualstudio.com/blogs/2022/11/28/vscode-sandbox), [Extension Host](https://code.visualstudio.com/api/advanced-topics/extension-host). No código atual, o serviço compartilhado é um `UtilityProcess` observado para crash, e o extension host local é iniciado como `WindowUtilityProcess` com ciclo de vida da Janela. [sharedProcess.ts](https://github.com/microsoft/vscode/blob/main/src/vs/platform/sharedProcess/electron-main/sharedProcess.ts#L147-L180), [extensionHostStarter.ts](https://github.com/microsoft/vscode/blob/main/src/vs/platform/extensions/electron-main/extensionHostStarter.ts#L72-L126) | É o precedente publicado mais próximo de um híbrido: coordenação e serviços estreitos globais, mas UI e extensões contidas por Janela. Não prova que um Projeto do VS Code tenha o mesmo modelo de estado do MyAlbuns. |
| **Blender** | A interface trabalha com um “arquivo atual”; abrir outro `.blend` substitui esse contexto e pode exigir confirmação de mudanças não salvas. A Recuperação também trata a última sessão/arquivo. [Topbar e arquivo atual](https://docs.blender.org/manual/en/4.2/interface/window_system/topbar.html), [Recuperação](https://docs.blender.org/manual/en/latest/troubleshooting/recover.html). O Blender também expõe execução headless para renderizar sem interface. [Argumentos de linha de comando](https://docs.blender.org/manual/en/5.0/advanced/command_line/arguments.html) | Modelo de produto próximo de um Projeto pesado por instância, embora as fontes consultadas não descrevam toda a árvore de processos da interface. A execução headless comprova uma fronteira explícita para trabalho pesado, não que a UI a crie automaticamente. |
| **GIMP 3** | O GIMP abre várias imagens e oferece modos de uma ou várias Janelas. [Janela de imagem](https://docs.gimp.org/3.0/en/gimp-concepts-main-windows.html). Plug-ins tradicionais são processos independentes iniciados pelo core e usam IPC; a documentação afirma que o crash do plug-in não derruba o core onde está a imagem. [Plug-ins C](https://developer.gimp.org/resource/writing-a-plug-in/tutorial-c-basic/). Filtros GEGL, por outro lado, são carregados no processo para oferecer preview imediato e efeitos não destrutivos. [Plug-ins e filtros](https://developer.gimp.org/resource/about-plugins/) | Host multi-imagem com isolamento seletivo. É a demonstração mais explícita de que latência e integração podem justificar código in-process, enquanto componentes mais falháveis recebem processo próprio. |
| **Krita 5.3** | O Krita abre novos documentos em uma instância já existente. A antiga opção de permitir múltiplas instâncias foi descontinuada porque seu banco SQLite de recursos não pode ser administrado por várias instâncias. [General Settings](https://docs.krita.org/en/reference_manual/preferences/general_settings.html). Sessões podem guardar imagens e múltiplas Janelas, e o aplicativo oferece autosave e recuperação. [Workspaces e sessões](https://docs.krita.org/en/reference_manual/resource_management/resource_workspace.html), [conceitos básicos e Recuperação](https://docs.krita.org/en/user_manual/getting_started/basic_concepts.html) | Próximo de B no host de documentos, motivado por uma fonte global compartilhada. A documentação consultada não publica uma árvore completa de helpers. Mostra que a política de armazenamento também pode determinar a topologia. |
| **Photoshop** | Um aplicativo organiza vários documentos em abas, Janelas flutuantes ou layouts lado a lado. [Janelas de documento](https://helpx.adobe.com/photoshop/desktop/get-started/learn-the-basics/rearrange-document-windows.html). O aplicativo mantém informação periódica para Recuperação após crash. [Recuperação de arquivos](https://helpx.adobe.com/photoshop/kb/file-recovery-photoshop.html) | Comprova sessão multidocumento e Recuperação, mas as fontes atuais consultadas não publicam o mapeamento completo de documentos, render, GPU e plug-ins para processos. Portanto, Photoshop não pode ser contado como prova de B interno. |

### Casos complementares

O AutoCAD oferece ambos os fluxos ao usuário: vários desenhos podem abrir em
abas na mesma instância (`SDI=0`), enquanto iniciar novamente o programa permite
instâncias administradas separadamente. [Múltiplos desenhos e
instâncias](https://help.autodesk.com/view/ACADWEB/ENU/?caas=caas/sfdcarticles/sfdcarticles/Double-clicking-a-DWG-file-opens-a-new-instance-of-AutoCAD.html).
Seu Drawing Recovery Manager reúne os desenhos que estavam ativos numa sessão
que falhou. [Backup, autosave e
Recuperação](https://help.autodesk.com/view/RSTR/2023/ENU/?caas=caas/sfdcarticles/sfdcarticles/Understanding-AutoCAD-backup-and-autosave-files.html).
O caso comprova que um produto maduro pode oferecer sessão compartilhada e
instâncias separadas, mas não publica a topologia completa dos seus helpers.

O LibreOffice é um caso documentado de forte reutilização da instância: para
um mesmo perfil de usuário, apenas o primeiro processo executa o trabalho e
lançamentos excedentes encaminham seus argumentos a ele. [Perfil de usuário do
LibreOffice](https://wiki.documentfoundation.org/UserProfile/pt-br). A API do
desktop mantém vários componentes em frames, e a recuperação de arquivos pode
ser desativada com `--norestore`. [Desktop
singleton](https://api.libreoffice.org/docs/idl/ref/singletoncom_1_1sun_1_1star_1_1frame_1_1theDesktop.html),
[parâmetros de inicialização](https://help.libreoffice.org/latest/en-US/text/shared/guide/start_parameters.html).
É um precedente próximo de B, condicionado por um perfil global exclusivo.

O Premiere permite vários Projetos abertos e, após crash, oferece reabrir os
Projetos disponíveis no estado recuperado. [Projetos
múltiplos](https://helpx.adobe.com/premiere/desktop/organize-media/create-projects/edit-multiple-open-projects.html),
[Recuperação](https://helpx.adobe.com/premiere/desktop/troubleshooting/crash-issues/recover-projects-after-a-crash.html).
O Final Cut Pro permite múltiplas bibliotecas abertas e separa por biblioteca
os locais de mídia, Cache e backup. [Bibliotecas
múltiplas](https://support.apple.com/guide/final-cut-pro/open-and-close-libraries-ver217d6e77d/mac),
[gestão de bibliotecas](https://support.apple.com/guide/final-cut-pro/intro-to-managing-libraries-ver07a37431a/mac).
Essas fontes comprovam fluxos multiprojeto, não a quantidade de processos; por
isso, os dois casos ficam fora da classificação interna A/B.

Como contraste, o Lightroom Classic mantém um único catálogo ativo: abrir
outro fecha o atual e relança o aplicativo. A Adobe recomenda normalmente um
catálogo, mas reconhece catálogos separados para segregação profissional e
redução do alcance de corrupção. [Gestão de
catálogos](https://helpx.adobe.com/lightroom-classic/help/create-catalogs.html),
[benefícios de um catálogo](https://helpx.adobe.com/lightroom-classic/help/single-catalog-benefits.html).
Esse modelo não satisfaz o requisito do MyAlbuns de vários Projetos abertos,
mas demonstra que a unidade de dados pode prevalecer sobre o modelo de
Janelas.

### Casos não classificáveis

Nas fontes oficiais consultadas, Illustrator, InDesign, DaVinci Resolve e
Affinity não forneceram evidência atual suficiente para mapear documentos ou
Projetos a processos locais. Há documentação de Janelas, GPU, Recuperação,
plug-ins ou render remoto em partes, mas combiná-las em uma topologia interna
seria especulação. Eles não participam de nenhuma conclusão de prevalência.

O suporte do InDesign, por exemplo, documenta que um documento de Recuperação
corrompido pode provocar um ciclo de crashes ao reabrir o aplicativo.
[Recuperação do
InDesign](https://helpx.adobe.com/indesign/desktop/troubleshoot/file-and-output-issues/recover-indesign-documents.html).
Isso evidencia a necessidade de Recuperação seletiva, mas não prova que todos
os documentos residam no mesmo processo.

## O que a amostra permite concluir

Não foi encontrada uma pesquisa representativa que contabilize topologias de
processo em editores profissionais. Assim, não há percentual defensável nem
uma maioria A/B.

Na seleção deliberada desta pesquisa, o fluxo multidocumento numa sessão
visível aparece em Photoshop, GIMP, Krita, AutoCAD, LibreOffice, Premiere e
Final Cut Pro. Isso é uma observação de produto, não uma votação sobre
processos. Blender e Lightroom mostram outro modelo de produto, no qual apenas
um arquivo ou catálogo fica ativo; as fontes consultadas não publicam a árvore
completa nem permitem atribuir esse modelo a uma causa arquitetural única.

Há evidência direta de processos para uma família híbrida de **coordenador
compartilhado com isolamento seletivo** em Chromium, Electron, VS Code e GIMP.
Krita e LibreOffice documentam forte reutilização da instância associada a
recursos globais, mas não publicam toda a árvore de helpers. Blender e Lightroom
são apenas contrastes de modelo de produto, e os demais casos não
classificáveis não entram nessa comparação interna.

A regularidade observada nos casos transparentes não é “um processo por
documento” nem “um processo para tudo”. É separar componentes conforme risco,
ciclo de vida e custo de compartilhamento. A granularidade correta para um
Projeto do MyAlbuns continua sendo uma decisão do próprio domínio e das
medições do produto.

## Por que compartilhar ou separar

### Razões para compartilhar

- evitar duplicação de runtime, memória, GPU e dados de perfil;
- manter configurações, catálogo de recursos e operações globais sob um único
  proprietário;
- reduzir inicialização, IPC e coordenação entre documentos;
- permitir caminhos de baixa latência, como os filtros GEGL com preview no
  Canvas.

### Razões para separar

- impedir que crash ou travamento de um componente interrompa outros
  documentos;
- limitar código de terceiros ou conteúdo menos confiável;
- reiniciar ou encerrar independentemente tarefas intensivas;
- atribuir CPU, memória, logs e diagnóstico a uma unidade identificável;
- alinhar o processo à unidade que o usuário consegue salvar e recuperar.

### Custos da separação

- mais processos, memória privada e tempo de inicialização;
- mais contratos IPC, correlação de logs e regras de encerramento;
- possível duplicação de runtimes, contextos gráficos e Cache;
- necessidade de coordenar exclusividades genuinamente globais.

Chromium torna esse trade-off explícito: usa processos para estabilidade,
segurança e paralelismo, mas conserva memória por reuso condicionado e um
limite ajustado aos recursos da máquina. Isso é um princípio, não uma razão
para introduzir agora um pool adaptativo no MyAlbuns sem telemetria.

Recuperação e isolamento são complementares. Um processo menor reduz o alcance
da falha; autosave ou Recuperação limita a perda dentro da unidade afetada.
Nenhum substitui o outro.

## WebView2: a fronteira que importa para o MyAlbuns

O WebView2 usa um grupo de processos com um browser, um ou mais renderers e
helpers como GPU e áudio. Ambientes configurados da mesma forma e usando o
mesmo User Data Folder podem representar a mesma coleção de processos, mesmo
quando criados por processos hospedeiros diferentes. [Modelo de processos do
WebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model).

Quando o processo principal do browser falha, todos os controles associados à
mesma configuração de ambiente são fechados e precisam ser recriados. Falhas
de renderer podem ser tratadas por recarga; GPU e certos utilitários são
normalmente recriados. [Eventos e Recuperação de
processos](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-related-events).

Separar User Data Folders cria custo: a Microsoft alerta que cada processo de
browser adicional consome memória e espaço e recomenda evitar UDFs demais.
[Gestão de User Data
Folders](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/user-data-folder).

Daí surgem duas inferências importantes para A:

- um executável por Projeto, sozinho, não garante isolamento completo se os
  ambientes WebView2 acabarem no mesmo grupo de processos;
- perfis no mesmo UDF podem separar dados de navegação, mas não equivalem a
  separar o domínio de falha do browser.

A implementação de produção deve tornar a identidade/configuração do ambiente
WebView2 parte explícita do isolamento, medir seu custo e limpar dados
descartáveis com ciclo de vida definido. Não deve criar UDFs ilimitados nem
pressupor isolamento apenas pela quantidade de Janelas.

## Implicações para o MyAlbuns

| Responsabilidade | Precedente observado | Direção coerente |
| --- | --- | --- |
| Boas-vindas, singleton e exclusividades globais | `main`/serviço global em Electron e VS Code | `MyAlbuns.exe` pequeno, sem estado criativo mutável |
| Sessão editável de um Projeto | isolamento por Janela/worker e modelos de um agregado por instância | um host A por `ProjectSession` |
| Canvas WebGL2 | renderer isolado, mas GPU potencialmente compartilhada | ambiente WebView2 identificado e falhas monitoradas |
| Cache e Exportação | utility processes, plug-ins independentes e render headless | `MyAlbuns.Imaging.exe` separado e reiniciável |
| Configurações e Catálogo global de Layouts | recursos globais de Krita/LibreOffice | proprietário global estreito, sem absorver estado do Projeto |
| Recuperação | mecanismos em vários casos, com granularidades de documento, Projeto ou sessão | checkpoint separado por Identidade do Projeto |

A alternativa A do MyAlbuns não significa duplicar o aplicativo inteiro. Ela
forma um híbrido:

- `MyAlbuns.exe` continua único e leve;
- cada Projeto possui uma autoridade mutável e um host próprios;
- o Processador de Imagens permanece uma fronteira separada;
- Configurações e exclusividades verdadeiramente globais permanecem globais.

Esse desenho se aproxima do princípio usado por VS Code, Electron e GIMP:
compartilhar coordenação, isolar UI ou trabalho falhável. A diferença é que o
MyAlbuns escolhe o **Projeto**, e não uma origem web ou plug-in, como unidade
recuperável principal.

B continua tecnicamente válida. Ela se aproxima dos hosts multidocumento e
reduz processos, mas uma falha no host nativo continua atingindo todas as suas
Janelas, mesmo que o WebView2 mantenha renderers separados. O spike final
observou exatamente essa diferença: em A, uma Janela e seu Projeto
sobreviveram à queda de outro host; em B, nenhuma Janela sobreviveu. A também
foi consistentemente melhor em Cache, Canvas pronto e navegação, enquanto B
usou menos processos e teve melhor Zoom/GPU em métricas específicas. Esses
resultados estão em [Comparação final das
topologias](0028-comparacao-final-de-topologias.md).

## Recomendação

Adotar **A no MVP, dentro da arquitetura híbrida já definida**, pelos seguintes
motivos cumulativos:

1. a Sessão do Projeto é a autoridade mutável, a unidade de bloqueio, de
   Salvamento e de Recuperação;
2. a falha de um host ficou contida ao Projeto correspondente no teste real;
3. A apresentou diferenças consistentes favoráveis em Cache pronto,
   duração/vazão do Cache, Canvas pronto e navegação;
4. os precedentes mais transparentes isolam componentes por risco e ciclo de
   vida; manter `MyAlbuns.exe` sem estado criativo é uma invariante própria do
   MyAlbuns, registrada no ADR, e não uma conclusão emprestada desses sistemas;
5. A usou 14 processos e B usou 8 no spike; as diferenças de working set e
   memória privada permaneceram inconclusivas. Aceitar essa contagem adicional
   no MVP é um julgamento arquitetural explícito, sujeito a monitoramento.

A recomendação não afirma que programas Adobe usem A, nem que A seja mais
popular. A evidência externa mostra que A cabe numa família arquitetural já
usada, mas sua granularidade por Projeto deriva dos requisitos e das medições do
próprio MyAlbuns.

Não convém manter A e B completas em produção. A reversibilidade deve vir de
interfaces neutras: `ProjectCore` independente da UI, `ProjectSession`
identificada, comandos roteados por Identidade do Projeto, Recuperação por
Projeto e orquestração concentrada no composition root.

## Quando reavaliar

B, ou uma variante com reuso controlado, só deve voltar à decisão quando
telemetria de uso real demonstrar pelo menos uma destas condições:

- quantidade típica de Projetos simultâneos suficiente para tornar o número de
  processos ou UDFs um problema recorrente;
- pressão de memória, GPU ou disco atribuída aos grupos WebView2 por Projeto;
- tempo de abertura dominado pela duplicação de runtime, e não por Cache ou
  mídia;
- custo operacional de IPC, logs e atualização dos hosts maior que o benefício
  observado de isolamento;
- mecanismo testado que preserve ou recupere de forma aceitável os Projetos não
  relacionados quando o host nativo compartilhado falhar; conter a queda de um
  renderer permanece um requisito adicional;
- repetição do benchmark em hardware representativo mostrando vantagem
  consistente de B nas métricas relevantes, sem ampliar perda ou corrupção.

Esses gatilhos são observáveis. Até que um deles apareça, implementar reuso
adaptativo, um pool de hosts ou as duas topologias simultaneamente seria
complexidade especulativa.

## Conclusão

Editores consolidados não convergem para uma relação universal entre Projeto,
Janela e processo. Vários aplicativos nativos consultados apresentam múltiplos
documentos numa sessão; runtimes baseados em Chromium e aplicativos como VS
Code introduzem isolamento por renderer ou Janela; GIMP separa plug-ins, mas
mantém filtros de baixa latência no processo; Blender e Lightroom documentam
modelos de um agregado ativo por instância.

Para o MyAlbuns, adotamos como critério de produto alinhar a fronteira de falha
à unidade que o usuário salva e recupera: o Projeto. Esse critério não é uma lei
da indústria; ele combina o domínio do produto com os resultados do spike. A
decisão resultante — **host independente por Projeto, launcher global leve e
Processador de Imagens separado** — é compatível com a família híbrida observada,
mas não é imposta por ela.

Esta pesquisa fornece subsídio para o último gate do ticket 01, mas não altera
por si só o status do [ADR 0005](../adr/0005-adotar-tauri-react-rust.md) nem o
ticket.

> **Decisão posterior:** o ADR 0005 foi aceito com a topologia A após a
> ponderação desta pesquisa, dos requisitos do MyAlbuns e das medições do spike.
