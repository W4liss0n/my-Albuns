---
status: current
document: research
date: 2026-09-08
ticket: 20
platform: windows
---

# Troca de conteúdo entre dois Frames

Este recorte continua a edição de Frames da issue #20. No Modo de edição da
Lâmina, `Editar > Trocar conteúdo dos Frames` e o mesmo comando no menu
contextual ficam disponíveis para exatamente dois Frames selecionados, desde
que ao menos um contenha Foto. Dois placeholders mantêm o comando desabilitado.
Não há novo atalho.

Com duas Fotos, cada ocorrência passa para o outro Frame. Com Foto e placeholder,
a Foto passa ao Frame vazio e a origem torna-se placeholder. A seleção dos
dois Frames permanece, inclusive ao desfazer e refazer. Posição, dimensões,
identidade, estilo e ordem dos Frames não são trocados.

## Autoridade e composição

`ProjectIntent::SwapFrameContents` recebe os identificadores capturados quando
o comando é acionado. O Core exige dois IDs distintos da mesma Lâmina e ao
menos uma Foto, usando a mesma validação de seleção da exclusão e da ordenação.
IDs repetidos, inválidos, removidos ou pertencentes a Lâminas diferentes
rejeitam a operação inteira, sem alterar o Projeto ou descartar Redo.

A operação transfere o `ProjectPhoto` inteiro. O vínculo e os ajustes
persistentes de Pan e Zoom acompanham a ocorrência, inclusive quando duas
ocorrências usam o mesmo Arquivo com ajustes diferentes. Os Arquivos originais,
os vínculos de mídia e seus contadores de uso permanecem iguais.

O compositor calcula o Preenchimento a partir da máscara de destino. O Pan
normalizado e o Zoom do usuário conservam seus valores; a escala base e o
deslocamento em coordenadas físicas são derivados novamente das dimensões
da Foto e do novo Frame. Isso mantém o preenchimento em proporções diferentes,
sem reiniciar o enquadramento como ocorre na substituição por outra Foto.

## Histórico e persistência

Uma troca constitui uma ação de Undo/Redo. A fila compartilhada mantém Salvar
e Desfazer atrás de uma troca pendente e usa a Revisão concluída. Se a troca
falhar, os comandos adjacentes dependentes são cancelados. Uma seleção feita
enquanto a troca está pendente permanece atual; concluir o comando não volta
a selecionar seu par original.

Salvar e reabrir preservam o resultado. A composição congelada da Exportação
recebe a mesma troca, enquanto uma captura anterior permanece estável. O
Arquivo de Projeto mantém o esquema 3, sem migração adicional.

## Verificação e limites

Os testes públicos do Core cobrem Foto/Foto, Foto/placeholder, ocorrências
independentes de um mesmo Arquivo, proporções extremas e limites de Pan,
rejeição atômica, Histórico, Salvamento, reabertura e composição congelada.
Os testes da interface exercitam ambos os menus, disponibilidade, seleção e
a fila com sucesso ou falha. A implementação usa React 19.2.8; a consulta
do contrato de atualização e handlers usou a documentação indexada 19.2.7.

O corpus `frame-content-swap-cases.json` é produzido e conferido pelo Core.
As prévias de aceitação visual consomem esses resultados nos componentes
React/Pixi. A conferência inclui os menus existentes afetados pelo novo
comando, além de troca, placeholder, disponibilidade, Desfazer e Refazer.
Evidências de execução e julgamentos visuais permanecem fora do controle
de versão. Testes automatizados com janelas nativas continuam suspensos.

O comando não depende de autorização para alterar geometria. O travamento
persistente de Layout, porém, ainda não é exposto pelo Core, e sua integração
com todos os comandos continua no recorte proprietário. Giro, Ângulo,
Espelhamento, efeitos e estilos individuais ainda não persistidos também
permanecem nas respectivas entregas. Cópia e colagem são o próximo recorte
da edição de Frames; a issue #20 permanece aberta.
