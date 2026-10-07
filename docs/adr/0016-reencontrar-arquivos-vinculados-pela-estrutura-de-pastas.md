---
status: accepted
date: 2026-10-06
---

# Reencontrar Arquivos vinculados pela estrutura de pastas do Projeto

O Projeto grava cada Arquivo vinculado pelo caminho absoluto ([ADR 0001](0001-vincular-arquivos-externos.md)). Uma pasta de trabalho copiada inteira para um computador fora da rede, com o Projeto e suas imagens, abria sem nenhuma imagem: os caminhos apontavam para um servidor que não respondia, as imagens ficavam indisponíveis e `Localizar imagens…` não era oferecido, porque só vale para Arquivo ausente. O myAlbuns antigo procurava primeiro a imagem ao lado do Projeto, e os modelos levados assim para outra máquina deixaram de funcionar. O autor decidiu em 06/10/2026 que o Projeto deve achar sozinho as imagens quando a estrutura de pastas ao redor dele continuar a mesma.

## Decisão

- **Regra.** Subindo a partir da pasta atual do Projeto, cada pasta cujo nome também nomeia uma pasta do caminho gravado indica um candidato: essa pasta seguida do que vinha depois do mesmo nome no caminho gravado. Os nomes são comparados sem diferenciar maiúsculas, acentos incluídos. A pasta mais próxima do Projeto vem primeiro, e, quando o nome se repete no caminho gravado, a ocorrência mais funda vem primeiro. A raiz (unidade ou compartilhamento) nunca serve de âncora, e um candidato igual ao caminho gravado é descartado. Vale o primeiro candidato que existe e pode ser lido.
- **Previsível.** Nada é listado nem pesquisado: não há busca em subpastas, em outros discos ou por outros nomes. Cada candidato é um caminho exato derivado do gravado, e as mesmas pastas sempre produzem o mesmo resultado. Um nome de arquivo igual sem a mesma estrutura não basta.
- **Estrutura primeiro.** Os candidatos são procurados antes do caminho gravado, e uma mídia encontrada não toca o caminho gravado. Assim, fora da rede, a abertura não espera o servidor. Com o servidor no ar, uma cópia na estrutura ao lado do Projeto vence o caminho gravado.
- **Em silêncio, ao abrir.** Logo depois da decisão de Recuperação, e antes de hidratação, Cache e Monitor, os caminhos encontrados substituem os do documento aberto. Não há aviso, passo de Undo/Redo, nova revisão nem mudança pendente; um Projeto recuperado continua com suas mudanças pendentes. O arquivo de Projeto só recebe os caminhos novos quando é salvo por outro motivo. Enquanto ninguém salvar, a busca se repete a cada abertura.
- **Sem duplicar vínculo.** Um caminho que outra mídia do mesmo tipo já usa não é adotado, como uma Religação recusaria.
- **Fotos e Imagens decorativas.** A regra vale para os dois tipos.
- **Lote e linha de comando.** A Exportação em lote procura pela estrutura do Projeto de cada item, a cada verificação, depois das religações temporárias escolhidas pelo usuário, e nunca salva o Projeto. A busca fica no planejamento do lote, não no `ExportPipeline`. A Geração em lote pela linha de comando aplica a mesma busca ao modelo, relativa à pasta do modelo, e os Projetos gerados recebem os caminhos encontrados; o modelo não é alterado. A geração pela janela usa o Projeto aberto, que já passou pela busca.

## Consequências

- Não é Religação: não exige escolha do usuário, não cria Histórico e não depende de o caminho gravado estar ausente. Religação, `Tentar novamente` e Substituir imagem continuam como antes.
- O formato do arquivo de Projeto não muda, e a regra vale também para Projetos salvos antes dela, inclusive os convertidos do myAlbuns antigo.
- Uma Foto pode deixar de usar o original no servidor quando existe uma cópia na estrutura ao lado do Projeto. Por exemplo, um Projeto copiado de um trabalho para outro, com as imagens também copiadas, passa a usar as imagens do trabalho novo.
- Uma unidade mapeada e o compartilhamento correspondente são grafias diferentes: um Projeto aberto por `Z:\` pode trocar `\\servidor\...` por `Z:\...` sem comparar identidade física, porque essa comparação tocaria o caminho gravado e traria de volta a espera pelo servidor.
- A prévia da primeira Lâmina em Projetos recentes continua usando o caminho gravado.
- A geração dos candidatos fica no módulo de caminhos (`structural_candidates`), e a verificação de existência passa pelo acesso centralizado aos Arquivos vinculados ([design 0052](../design/0052-acesso-aos-arquivos-vinculados.md)).
