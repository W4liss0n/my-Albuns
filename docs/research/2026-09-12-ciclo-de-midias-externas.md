---
status: current
document: research
date: 2026-09-12
ticket: 19
---

# Ciclo de mídias externas: recuperação na Exportação

A entrega complementa o [item #19](https://github.com/W4liss0n/my-Albuns/issues/19)
com recuperação dos Originais na Exportação individual e confirmação completa
de mudanças externas em Decorativos. O item permanece aberto: a integração de
`Relinkar todos` depende da Exportação em lote, ainda prevista no
[item #39](https://github.com/W4liss0n/my-Albuns/issues/39).

## Comportamento entregue

Antes de escolher o Destino ou iniciar o processamento final, a Exportação da
Lâmina verifica somente seus Originais necessários. Ausência ou
indisponibilidade abre a tabela de Problemas. Arquivos sem uso naquela seleção
não bloqueiam a operação.

`Relinkar` solicita a pasta das Fotos e procura nas subpastas pelo nome e pela
extensão exatos. Somente uma correspondência única é proposta para cada
referência ausente; zero ou várias correspondências permanecem pendentes. A
inspeção completa confirma o candidato antes da alteração. Falhas de leitura
da árvore impedem afirmar unicidade; atalhos simbólicos e junções interrompem
a busca com um motivo explícito, evitando percursos incompletos ou cíclicos.

Cada referência corrigida usa o comando normal da Sessão, participa de
Undo/Redo e fica pendente de Salvamento manual. As prévias passam pelo mesmo
processamento de imagens. A Exportação já pode usar a sessão corrigida; o
arquivo do Projeto permanece intacto até Salvar.

`Tentar novamente` reinspeciona as origens sem alterar referências, Histórico
ou dirty. Resolver a última pendência não inicia a Exportação: é necessário
clicar em `Continuar Exportação`. Fechar a tabela cancela a tentativa pendente
e conserva as religações já feitas na sessão.

O Monitor agora exige a leitura completa também para Decorativos antes de
confirmar conteúdo novo e invalidar o Cache. Uma gravação externa parcial
conserva a observação anterior; uma gravação válida atualiza as ocorrências
abertas sem criar alterações criativas.

## Cobertura do item

| Grupo de critérios do #19 | Situação |
| --- | --- |
| Importação de JPEG, PNG e TIFF nas duas abas; arquivos, pastas e arraste; Histórico e resultados parciais | Já conectado ao fluxo produtivo, com expansão não recursiva e testes nativos da importação. |
| Vínculos externos, identidade funcional por aba, Cache descartável e reabertura | Implementação existente preservada. |
| Monitor, ausência, indisponibilidade e atualização externa | Confirmação completa estendida a Decorativos; Fotos mantêm o percurso validado anteriormente. |
| Religação individual no Painel | Percurso existente reutilizado; inspeção e orçamento passam a compartilhar o plano de raízes capturado. |
| Religação por pasta na Exportação individual e bloqueio limitado à seleção | Implementado nesta entrega sobre a Exportação de Lâmina disponível. |
| `Relinkar todos`, pasta com Nome exato do Projeto e mapa temporário do lote | Pendente da interface e do executor do #39; não há comando fictício nem alteração automática de Projetos em lote. |

As ampliações dos formatos e intervalos da Exportação pertencem ao
[item #35](https://github.com/W4liss0n/my-Albuns/issues/35). Esta entrega usa a
ação produtiva `Exportar Lâmina`, preservando o contrato do renderer: recebe
um snapshot imutável e não executa Religação nem Salvamento.

## Roteiro de teste

1. Em um Projeto de teste, importe duas imagens e coloque apenas uma na Lâmina.
2. Salve o Projeto. Mova a imagem que ficou somente no Painel: a Lâmina ainda
   deve exportar.
3. Mova também a imagem usada para uma subpasta de outra pasta. Ao exportar,
   confira a indicação de ausência e clique em `Relinkar`.
4. Escolha a pasta que contém a nova localização. A prévia deve ser preparada,
   o Projeto deve ficar com alterações pendentes e a tabela deve aguardar
   `Continuar Exportação`.
5. Feche a tabela e experimente Undo/Redo. Reabra a Exportação e confirme que
   a referência corrigida funciona antes de Salvar.
6. Repita com duas cópias de mesmo nome e extensão em subpastas diferentes:
   a busca deve manter a pendência, sem escolher uma arbitrariamente.
7. Para indisponibilidade, use uma origem temporariamente inacessível. A ação
   deve ser `Tentar novamente`, preservando o vínculo.

## Evidências

O teste com arquivos temporários reais cobre a seleção de dependências,
ausência, objeto de tipo incorreto, busca recursiva, snapshot com referência
não salva, preservação dos bytes persistidos e Undo/Redo. Outro percurso usa
dois Hosts para Fotos e Decorativos, comparando uma gravação truncada e sua
sucessora válida, sem alterações de Histórico.

Os testes da interface cobrem a espera pela confirmação, a exclusão entre
ações durante a recuperação e a separação entre Religação e nova inspeção.
As capturas declaradas estão no manifesto de aceitação visual; relatórios
locais ficam em `.scratch/ui-acceptance/media-delivery/`.

Os contratos externos foram consultados com `find-docs`, usando a fonte
oficial como alternativa à indisponibilidade da cota do Context7: Rust 1.98.0,
[enumeração de diretórios](https://doc.rust-lang.org/std/fs/fn.read_dir.html),
[tipos de arquivo](https://doc.rust-lang.org/std/fs/struct.FileType.html) e
[metadados Windows](https://doc.rust-lang.org/std/os/windows/fs/trait.MetadataExt.html).
As páginas publicadas correspondem a 1.98.1 e documentam as APIs estáveis
usadas. O diálogo foi conferido no código local da versão fixada
`tauri-plugin-dialog` 2.7.2.
