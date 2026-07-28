---
status: accepted
date: 2026-07-28
---

# Publicar Exportações com garantia transacional limitada

Uma Exportação pode gerar muitos arquivos e substituir uma saída anterior. O sistema precisa evitar anunciar sucesso com arquivos ainda incompletos e não pode remover saídas órfãs antes de saber que o novo conjunto foi produzido. Ao mesmo tempo, um diretório comum escolhido pelo usuário não oferece uma transação atômica para o conjunto inteiro.

## Decisão

Cada Exportação segue duas fases:

1. **Preparação**: todas as novas saídas selecionadas são renderizadas e verificadas em uma pasta reservada dentro da própria pasta de Destino, sem modificar o conjunto final.
2. **Publicação**: os arquivos preparados são promovidos um a um, usando substituição atômica por arquivo quando o sistema de arquivos oferecer essa operação.

Uma falha ou cancelamento durante a preparação remove a tentativa quando possível e preserva o conjunto final anterior.

Depois que a publicação começa, não existe garantia de rollback atômico do conjunto inteiro. Uma falha, remoção do Destino, falta de energia, corrupção física, interferência do sistema operacional ou modificação externa concorrente pode deixar uma combinação de arquivos anteriores e novos. Nessa situação:

- a operação termina como falha e nunca é apresentada como concluída;
- a interface informa que o Destino pode conter uma publicação parcial e orienta o usuário a tentar novamente;
- artefatos temporários são removidos quando isso for seguro e possível;
- nenhuma Saída órfã é apagada;
- nenhum manifesto permanente é criado no Destino.

Saídas órfãs elegíveis são removidas somente depois que todos os arquivos planejados do Projeto forem publicados com sucesso. A limpeza continua limitada à convenção de nomes do ADR 0003.

Na implementação, o `ExportPipeline` possui o ciclo de vida da preparação e garante sua limpeza nos estados terminais tratáveis. Sua fase `Publisher` possui a promoção aos nomes finais e a limpeza de órfãos permitida. Manter o staging no próprio Destino viabiliza a substituição por arquivo quando suportada, mas não cria backup integral, manifesto ou rollback do conjunto.

## Consequências

- O usuário nunca recebe sucesso antes de todas as saídas planejadas estarem no Destino.
- Falhas antes da publicação preservam a saída anterior; falhas durante a publicação possuem um envelope explicitamente limitado.
- Não são necessários backups integrais nem um protocolo de recuperação permanente no Destino.
- Uma nova Exportação integral para o mesmo Destino é o caminho para restabelecer um conjunto coerente depois de uma publicação parcial.
- Destinos locais, UNC, em unidade mapeada ou verbatim local/UNC usam o mesmo contrato; o suporte real à substituição atômica continua sendo verificado pela operação do sistema de arquivos.
- Testes devem distinguir falha de preparação, falha antes da primeira promoção, falha entre duas promoções, queda do Processador e falha durante a limpeza de temporários.
