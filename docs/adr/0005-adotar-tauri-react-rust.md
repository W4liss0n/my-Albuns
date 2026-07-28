---
status: proposed
date: 2026-07-28
---

# Validar Tauri 2, React/TypeScript e Rust como arquitetura principal

O MyAlbuns será inicialmente um aplicativo para Windows 10/11 x64. A primeira versão precisa combinar uma interface desktop rica, um Canvas acelerado por hardware, estado criativo confiável, múltiplos Projetos isolados e um pipeline final independente da prévia e do Cache.

Tauri 2 com React/TypeScript e Rust é a hipótese arquitetural principal. Ela ainda não é uma escolha irreversível: um spike vertical deve validar a stack e comparar as duas topologias de hospedagem descritas neste ADR antes de o status mudar para `accepted`.

## Direção proposta

- React/TypeScript hospeda a interface e apenas o estado transitório das interações.
- PixiJS sobre WebGL2 compõe a prévia interativa.
- Um núcleo Rust compartilhado, denominado provisoriamente `myalbuns-core`, expõe o seam externo `ProjectCore` com duas entradas: abrir uma sessão editável e carregar uma revisão persistida somente para leitura. Uma `ProjectSession` por Projeto aberto é a única proprietária mutável do estado criativo, e a entrada somente de leitura nunca instancia uma.
- `MyAlbuns.Imaging.exe` decodifica originais, produz a representação reduzida do Cache e renderiza Exportações a partir de um `RenderSnapshot` imutável e validado.
- `MyAlbuns.exe` hospeda Boas-vindas e as exclusividades globais estritamente necessárias, sem possuir estado criativo mutável de Projetos.

`myalbuns-core` deve ser independente de Tauri, React, PixiJS e do Processador de Imagens. Tanto o host interativo quanto o caminho headless do lote usam a interface do `ProjectCore` para carregar e migrar o documento, resolver sua Identidade antes de Cache ou Recuperação, validar invariantes e produzir o `RenderSnapshot`. `MyAlbuns.Imaging.exe` nunca interpreta por conta própria o arquivo bruto do Projeto.

As duas entradas compartilham carregamento, migração, resolução de Identidade e validação de invariantes. Somente a entrada editável instancia uma `ProjectSession` com Histórico, revisão corrente e mudanças pendentes; a entrada somente de leitura devolve um valor imutável suficiente para produzir o `RenderSnapshot` e nada mais. É essa separação que permite ao `BatchRunner` executar dentro do `MyAlbuns.exe` sem que esse processo passe a hospedar estado criativo mutável, e que impede uma falha do lote de gravar no arquivo do usuário.

Quando uma tentativa dependente de caminhos atravessar processos, seu proprietário envia também o `RootBindingPlan` imutável daquela tentativa. O processo receptor usa os mesmos bindings e não resolve novamente por conta própria unidades mapeadas ou raízes já capturadas.

O frontend pode manter seleção, hover e transformações transitórias enquanto um gesto ocorre, mas envia uma única intenção consolidada ao núcleo quando o gesto termina. Não deve existir uma segunda implementação das regras do Projeto em TypeScript.

O critério para separar as duas responsabilidades é o efeito da regra, não o lugar onde o gesto acontece: **se o resultado sobrevive ao Salvamento ou participa do Undo/Redo, a regra pertence ao núcleo**. Caso contrário, pertence à interface. O critério é mecânico e dispensa julgar caso a caso.

Por ele, escolher qual Frame recebe uma Foto, qual Frame responde a um ponto com sobreposição e onde uma colagem é posicionada são regras do núcleo, ainda que só sejam alcançadas por gestos. A interface reporta o gesto — a Foto, a Lâmina e o ponto — e recebe de volta a decisão. Já o nível de Zoom do Canvas, o realce de hover e o limiar de arraste da plataforma pertencem à interface, porque nada do que produzem é salvo.

## Propriedade e módulos

A interface externa do `ProjectCore` esconde inicialmente três responsabilidades internas: `ProjectDomain` mantém tipos, invariantes e comandos; `ProjectSession` mantém estado atual, revisão, mudanças pendentes e Undo/Redo; `ProjectStore` mantém versão, migração, validação estrutural e Salvamento atômico. O spike pode alterar nomes e empacotamento, mas não pode criar dois proprietários mutáveis do mesmo Projeto.

Um `CompositionCore` puro recebe valores imutáveis e produz planos determinísticos de recorte, preenchimento, transformação e ordem de desenho. Editor e Exportação reutilizam essa regra sem duplicá-la em TypeScript ou no Processador. A transformação persistente da Foto dentro do Frame é diferente da transformação transitória usada para navegar no Canvas.

Referências persistentes de mídia pertencem ao Projeto. Disponibilidade observada, watcher e Cache não participam do estado criativo. Um `CacheEngine` é o proprietário lógico de jobs, índice, artefatos, invalidação, pausa e manutenção; o Processador de Imagens pode ser seu único adaptador escritor sem se tornar fonte canônica.

A Exportação usa um único `ExportPipeline` para o fluxo normal e cada item do lote. Planejamento, execução e Publicação são fases internas; o lote permanece um chamador externo responsável por descoberta, pré-validação e checkpoint. O pipeline recebe snapshot imutável, abre originais e nunca salva, religa ou modifica um Projeto.

Exclusividade global é representada por um `OperationGate` pequeno. Cancelamento e progresso pertencem a cada tentativa, e a pausa do Cache pertence ao `CacheEngine`; não existe um coordenador universal de comandos, estado de interface e operações.

Stores de Projeto, Configurações, Layouts, Estado, Recuperação e Cache conservam políticas próprias. Eles podem compartilhar primitivas internas de escrita de um único arquivo, mas não são achatados em um armazenamento genérico.

O detalhamento e os nomes de trabalho estão em [Propriedade de estado e módulos do núcleo](../design/0012-propriedade-de-estado-e-modulos-do-nucleo.md). Eles orientam o spike sem transformar subdivisões internas em interfaces públicas prematuras.

## Topologias a comparar

O spike deve implementar o menor esqueleto suficiente das duas alternativas:

### A — host independente por Projeto

Cada Projeto aberto possui um processo `MyAlbuns.Project.exe` próprio, com uma Janela e uma instância isolada do núcleo. A falha de um host tende a ficar restrita àquele Projeto, ao custo de mais processos, WebViews, contextos gráficos, IPC, logs e memória.

### B — host multiwindow

Um único `MyAlbuns.Project.exe`, ou host equivalente, mantém várias Janelas e sessões isoladas. A alternativa tende a reduzir processos e duplicação de runtime, mas aumenta o domínio de falha e exige provar que estado, comandos, Cache e recursos gráficos nunca vazam entre Projetos.

A existência de Janelas separadas para o usuário é obrigatória nas duas alternativas. A quantidade de processos por Janela não é comportamento de produto e será escolhida pelas evidências do spike.

## Processamento de imagens

Durante a edição, `CacheEngine` agenda o trabalho descartável e cada sessão usa um Processador de Imagens isolado conforme a topologia escolhida. O adaptador executa Cache ou Exportação, nunca as duas atividades pesadas ao mesmo tempo; Exportação tem prioridade e usa os originais.

A Exportação em lote da primeira versão é exclusiva e serial. Um `BatchRunner` em `MyAlbuns.exe` carrega e valida um Projeto por vez pela entrada somente de leitura do `ProjectCore`, sem abrir sessão editável, chama o mesmo `ExportPipeline` do fluxo normal e inicia um único `MyAlbuns.Imaging.exe` temporário para aquele item. Paralelismo entre Álbuns, calibração automática e Perfil de desempenho ficam fora do MVP até existirem medições que justifiquem a complexidade.

O baseline de Cache é uma única representação reduzida por Foto ou Decorativo. Tiles, pirâmides e previews persistidos de Lâmina só podem ser introduzidos se as medições do spike mostrarem que o baseline não atende.

O modelo lógico conserva todas as Lâminas de um Álbum, sem um limite arbitrário. PixiJS materializa detalhes e texturas apenas para o viewport e uma margem de pré-carga; o spike deve medir descarte, reconstrução e navegação em Álbuns longos.

## Falhas e estado degradado

Não haverá eleição automática entre Janelas nem reinício automático de `MyAlbuns.exe` no MVP.

Se a topologia escolhida permitir que uma Janela de Projeto sobreviva à queda de `MyAlbuns.exe`, edição e Salvamento locais podem continuar; ações globais permanecem indisponíveis até o usuário relançar explicitamente o processo principal, protegido por singleton. Se a alternativa multiwindow não permitir essa sobrevivência, a Recuperação separada por Identidade deve limitar a perda. O spike registra a diferença em vez de pressupor um resultado.

A queda do Processador durante Cache descarta o trabalho incompleto e permite reconstruí-lo dos originais. Durante Exportação, a tentativa falha com segurança e nunca é apresentada como concluída apenas porque parte da saída existe.

## Requisitos gráficos e de segurança

WebGL2 acelerado por hardware é requisito do editor. Criar um contexto não basta: o diagnóstico deve confirmar um backend de hardware. Contexto ausente, rasterizador de software ou verificação inconclusiva preservam Boas-vindas, Configurações e diagnóstico, mas não abrem o editor. Não haverá fallback de edição por Canvas 2D ou software na primeira versão.

O instalador usará inicialmente WebView2 Evergreen e verificará sua disponibilidade. Capabilities, permissions e scopes do Tauri devem ser mínimos e explícitos; o frontend não recebe acesso genérico ao sistema de arquivos nem permissão genérica para iniciar processos.

## Validação obrigatória

O spike deve produzir evidência reproduzível para:

- uma Lâmina realista com Fotos grandes, Frames, máscara, Pan, Zoom, Overlay, seleção e Undo/Redo;
- feedback contínuo no Canvas e um único commit de domínio por gesto;
- uso do mesmo `myalbuns-core` no host interativo e no carregamento headless;
- uma única `ProjectSession` mutável por Projeto, sem estado criativo duplicado no frontend, watcher, Cache ou Processador;
- `CompositionCore` determinístico exercitado pela prévia e pela Exportação, distinguindo transformação da Foto de navegação do Canvas;
- `RenderSnapshot` validado como única entrada criativa do Processador de Imagens, sem leitura autônoma do documento pelo Processador;
- Cache, prévia e Exportação separados, com a saída final lendo somente originais;
- `CacheEngine`, `ExportPipeline` e um `OperationGate` realmente global nas duas topologias, sem concessões concorrentes e com liberação em sucesso, falha, cancelamento ou queda do proprietário, sem formar um coordenador universal;
- comparação A/B de memória, quantidade de processos, tempo de abertura, latência do Canvas, IPC, logs, empacotamento, recuperação e domínio de falha;
- duas ou mais Janelas de Projeto e isolamento observável de seus estados em ambas as topologias;
- perda e recuperação do contexto WebGL2, limites de textura e pressão de memória gráfica;
- uma representação reduzida por mídia como baseline, prototipando tiles somente diante de falha medida;
- navegação contínua e política de residência de texturas em um Álbum longo;
- contexto ausente, rasterizador de software e backend inconclusivo;
- capabilities e scopes restritos, WebView2 Evergreen, teste ponta a ponta e instalador `win-x64` em máquina limpa;
- responsividade durante geração de Cache e Exportação exclusiva;
- medições de tempo, vazão e memória com o hardware de cada execução registrado.

Os gates funcionais são binários. Metas quantitativas são congeladas no relatório antes da execução final de aceitação e não podem ser reajustadas depois de conhecido o resultado.

O relatório encerra o spike recomendando uma topologia e registrando os custos observados. A escolha só se torna normativa quando este ADR for atualizado para `accepted`; se nenhuma alternativa satisfizer os gates, um novo ADR avalia a contingência WPF/.NET com C# antes de qualquer implementação paralela.

## Consequências

- A stack favorece uma interface React e concentra domínio e processamento de imagens em Rust.
- O núcleo compartilhado reduz o risco de editor e lote interpretarem documentos de maneiras diferentes.
- Uma única sessão mutável e cálculos puros reduzem o risco de divergência entre estado salvo, prévia, Cache e Exportação.
- A alternativa por processo oferece domínio de falha menor, enquanto a multiwindow pode reduzir memória e complexidade operacional; o spike decide com evidência.
- A solução exige duas toolchains, contratos TypeScript/Rust, empacotamento de sidecar, logs correlacionados e testes reais no WebView2.
- O desempenho depende do WebGL2 disponível e precisa ser medido com cenas representativas.

## Decisões adiadas

- topologia A ou B;
- nomes finais, crates e visibilidade pública das subdivisões internas;
- transporte e esquema concretos entre processos;
- WebGPU no frontend e `wgpu` no pipeline Rust;
- bibliotecas concretas de codecs, ICC, EXIF, TIFF e PDF;
- formato e extensão do arquivo de Projeto;
- formato e resolução da representação reduzida e eventual adoção de tiles;
- números de threads e orçamento de memória;
- paralelismo futuro da Exportação em lote;
- estratégia para macOS;
- política de atualização do aplicativo.
