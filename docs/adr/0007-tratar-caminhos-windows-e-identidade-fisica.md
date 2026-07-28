---
status: accepted
date: 2026-07-28
---

# Tratar caminhos Windows como valores nativos e separá-los da identidade física

O MyAlbuns precisa abrir Projetos, mídias e destinos em discos locais, compartilhamentos UNC, unidades mapeadas e caminhos longos. Comparar ou normalizar esses caminhos como texto não é suficiente: representações diferentes podem alcançar o mesmo arquivo, uma origem de rede pode ficar temporariamente indisponível e a Localização do Projeto continua diferente de sua Identidade.

## Decisão

Um módulo Rust compartilhado possui a descoberta das pastas do aplicativo, a classificação e resolução de caminhos e a criação de locais temporários. React não monta caminhos nem recebe acesso genérico ao sistema de arquivos; os demais processos consomem valores já classificados pelo módulo.

O módulo usa valores nativos de caminho e aceita entradas totalmente qualificadas nos formatos locais, UNC, unidade mapeada, verbatim local e verbatim UNC. Caminhos relativos dependentes do diretório corrente, namespaces de dispositivos, curingas, fluxos alternativos de dados e componentes ou aliases reservados não são aceitos como Localização de Projeto, Arquivo vinculado ou Destino. Depois de abrir um alvo existente, o módulo confirma que ele é um objeto de disco do tipo esperado pelo chamador, como arquivo regular ou diretório. Para um alvo novo, valida o diretório pai e confirma pelo handle o objeto depois da criação.

A representação textual de um caminho nunca se torna identidade física. Para objetos existentes, o módulo pode comparar identidades fornecidas pelo sistema operacional e retornar `Same`, `Different` ou `Indeterminate`. A política diante do resultado pertence ao chamador: para impedir duas sessões editáveis do mesmo Projeto, o fluxo de abertura falha de forma fechada diante de `Indeterminate` e mantém o bloqueio obtido sobre o arquivo como proteção final.

Cada Importação, geração de Cache, Religação, Exportação ou operação em lote cria no componente proprietário um contexto temporário de resolução pertencente à tentativa lógica. A primeira ocorrência de cada raiz pode resolver, por exemplo, uma unidade mapeada para seu compartilhamento; esse binding operacional fica capturado durante a tentativa, e caminhos seguintes sob a mesma raiz reutilizam a resolução.

Quando todas as fases executam no mesmo processo, o contexto permanece local. Antes de a mesma tentativa atravessar host e Processador de Imagens, o proprietário congela o contexto em um `RootBindingPlan` imutável com as raízes conhecidas e o envia somente aos participantes pela IPC. Assim, processos diferentes não resolvem independentemente a mesma unidade mapeada durante uma tentativa. O plano não é Cache, não é global, não é persistido e é descartado junto da operação. Os bindings permanecem fixos enquanto a tentativa estiver ativa; qualquer falha a encerra, e `Tentar novamente` cria outro contexto e resolve o estado atual.

“Binding operacional capturado” significa conservar a associação escolhida, como `Z:` para a raiz UNC resolvida, e impedir que um remapeamento posterior da letra redirecione o trabalho. Não é promessa de fixar o servidor físico por trás de DFS, DNS, cluster ou SMB.

Reutilizar uma resolução não substitui o acesso ao arquivo. Exportação, Photoshop, validação final e qualquer outra ação autoritativa continuam abrindo o Arquivo vinculado original ou o Destino real. Falha de rede não é convertida automaticamente em Arquivo ausente.

## Consequências

- O Cache de mídia permanece sob a raiz local do aplicativo e nunca acompanha a localização de Projeto, mídia ou Exportação.
- Uma operação observa bindings consistentes inclusive quando atravessa processos, sem introduzir expiração, limpeza ou sincronização de um cache global.
- Caminhos de apresentação podem ser simplificados quando isso for seguro, mas a forma exibida não participa de identidade ou autorização.
- Bibliotecas concretas permanecem substituíveis atrás da interface do módulo e são avaliadas em pesquisa e no spike arquitetural.
