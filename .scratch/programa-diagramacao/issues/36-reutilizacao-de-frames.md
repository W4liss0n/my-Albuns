# 36 — Reutilização de Frames

**What to build:** permitir reutilizar composições por cópia e colagem de Frames e trocar o conteúdo entre duas posições sem alterar a geometria de seus Frames.

**Blocked by:** 21 — Layout travado.

**Type:** implementation

**Status:** ready-for-agent

**Normative sources:** [Especificação do produto](../../../docs/specs/programa-de-diagramacao-de-albuns.md); [estrutura da Janela do Projeto](../../../docs/design/0001-estrutura-da-janela-do-projeto.md).

- [ ] No Modo de edição, oferecer `Editar > Copiar`, `Editar > Colar` e os atalhos fixos do MVP `Ctrl + C` e `Ctrl + V`; copiar exige ao menos um Frame selecionado e colar exige conteúdo de Frame copiado.
- [ ] Copiar captura geometrias, ordem relativa, Fotos ou placeholders, estilos e ajustes não destrutivos de toda a Seleção de Frames, sem alterar o Projeto ou criar uma ação no Histórico.
- [ ] A área de transferência de Frames pertence somente à Janela de Projeto atual: permanece disponível ao navegar entre suas Lâminas, mas copiar em um Projeto nunca habilita colagem em outro.
- [ ] Colar na própria Lâmina calcula o deslocamento efetivo como o menor valor entre o deslocamento desejado e o máximo viável que mantém todo o conjunto na superfície ativa; colar em outra Lâmina equivalente preserva as posições.
- [ ] Se o deslocamento efetivo for zero, colar na mesma posição, colocar deterministicamente as novas instâncias acima das originais na Pilha visual, preservar sua ordem relativa e selecionar somente os Frames criados.
- [ ] Ao colar de Lâmina dupla em Página única, mapear proporcionalmente posições e dimensões de todo o conjunto para a Página ativa.
- [ ] Ao colar de Página única em Lâmina dupla, mapear proporcionalmente o conjunto somente para a Página do mesmo lado lógico: direita permanece direita e esquerda permanece esquerda.
- [ ] Toda colagem preserva relações internas, Fotos ou placeholders, ordem, estilos, ajustes e vínculos com os mesmos Arquivos originais, sem duplicar mídia ou Cache.
- [ ] Os Frames criados substituem a seleção anterior, e toda a colagem constitui uma única ação de Undo/Redo.
- [ ] Em Layout travado, copiar continua permitido e colar fica indisponível porque criaria novas estruturas de Frame.
- [ ] Oferecer `Trocar conteúdo dos Frames` em `Editar` e no menu de contexto quando exatamente dois Frames estiverem selecionados e ao menos um deles contiver Foto; dois placeholders desabilitam o comando.
- [ ] Com duas Fotos, trocar entre os Frames suas ocorrências completas; com uma Foto e um placeholder, mover a Foto para o placeholder e deixar o Frame de origem como placeholder.
- [ ] A troca preserva posição, dimensões, ordem e estilo de cada Frame; cada Foto leva seu Arquivo vinculado e todos os ajustes não destrutivos.
- [ ] Quando as geometrias forem diferentes, recalcular e limitar o enquadramento somente no necessário para preservar o Preenchimento do Frame sem áreas vazias.
- [ ] A troca funciona também em Layout travado e constitui uma única ação de Undo/Redo.
- [ ] Cópia, colagem e troca sobrevivem a `Salvar`/reabrir e produzem o mesmo resultado na Exportação.
- [ ] Testes cobrem seleção simples e múltipla, mesma Lâmina, Lâminas equivalentes, os dois sentidos entre Lâmina dupla e Página única, tentativa entre Projetos, Layout travado, duas Fotos, Foto com placeholder, dois placeholders, vínculos externos e atomicidade do Histórico.
