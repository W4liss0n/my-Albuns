---
document: research
date: 2026-09-11
ticket: 23
status: current
---

# Integração com Photoshop

Implementação do Programa 33, conforme a especificação e os contratos aceitos
0009, 0010, 0011 e 0012. Este registro técnico não altera esses contratos.

A janela Configurações está disponível em Boas-vindas e no menu Ferramentas.
A aba Photoshop permite escolher uma instalação detectada ou localizar
`Photoshop.exe`. Na ausência de uma preferência válida, a versão numérica mais
recente é selecionada. A escolha fica no arquivo `State/photoshop.json` da raiz
local fornecida por `AppPaths`, com versão de esquema, revisão e gravação atômica.
Não faz parte do Arquivo de Projeto nem do Histórico. As janelas consultam a
preferência ao abrir, recuperar o foco ou atualizar manualmente; não há broadcast
de preferências entre processos no MVP, conforme a revisão vigente do design 0010.

O menu da Foto no Painel e do Frame preenchido oferece **Abrir no Photoshop**.
`Ctrl+E` utiliza a mesma seleção contextual. Seleções múltiplas, Decorativos e
placeholders não iniciam a integração. O Host resolve novamente o identificador
recebido no Projeto atual e entrega somente o Original vinculado ao lançador.
Não exporta o Frame, não aplica o enquadramento e não usa uma representação de Cache.

O Monitor consolida observações sucessivas e prepara a alteração antes de atualizar
o MediaRuntime. Além da estabilidade dos metadados, verifica acesso de leitura;
um arquivo bloqueado para leitura ou temporariamente vazio permanece indisponível,
preservando a representação anterior. Fotos alteradas passam pela inspeção
completa existente, com admissão por memória, antes da confirmação e da invalidação.
Um JPEG parcial conserva a última observação e prévia válidas. A origem é conferida
novamente ao confirmar; resultados atrasados não substituem uma geração mais nova.
Cada Host observa suas ocorrências, inclusive quando
mais de um Projeto utiliza o mesmo Original. A inspeção e o processamento de
imagens continuam nos proprietários existentes, com admissão por memória.

## Contratos externos consultados

Ambiente: Rust 1.98.0, Tauri 2.11.5, tauri-plugin-dialog 2.7.2 e windows-sys 0.61.2.
O Context7 atingiu sua cota; a consulta prosseguiu nas fontes oficiais.

- [App Paths do Windows](https://learn.microsoft.com/en-us/windows/win32/shell/app-registration): pesquisa no Registro por usuário e máquina, nas visões de 32 e 64 bits. São consultados também os registros Adobe e os diretórios diretos de instalação em Program Files.
- [GetFileVersionInfoW](https://learn.microsoft.com/en-us/windows/win32/api/winver/nf-winver-getfileversioninfow) e [VerQueryValueW](https://learn.microsoft.com/en-us/windows/win32/api/winver/nf-winver-verqueryvaluew): validação do produto Adobe Photoshop e versão numérica do executável; o nome da pasta não decide compatibilidade.
- [Command](https://doc.rust-lang.org/std/process/struct.Command.html) e [CommandExt para Windows](https://doc.rust-lang.org/std/os/windows/process/trait.CommandExt.html): executável explícito e argumento de caminho nativo, sem montar uma linha de comando de shell.
- [Process creation flags](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags): o processo externo pode sobreviver ao Job do MyAlbuns, que permite breakaway no supervisor de desenvolvimento.
- [WebviewWindowBuilder do Tauri 2.11.5](https://docs.rs/tauri/2.11.5/tauri/webview/struct.WebviewWindowBuilder.html): Configurações é uma janela independente do Global, com permissões específicas. Pedidos de outros Hosts utilizam a ativação única já existente.

## Verificação

Testes do serviço cobrem descoberta, ordenação numérica, preferência persistida,
localização manual, aliases físicos, remoção da instalação, falha de lançamento,
Originais ausentes ou bloqueados e caminhos Unicode com mais de 260 caracteres.
O Host verifica a seleção sem mudar a projeção ou o estado de Salvamento.
O Monitor é exercitado com dois Projetos e várias ocorrências do mesmo Original,
durante bloqueio de leitura, truncamento e reaparecimento estável.

Testes da interface cobrem seleção explícita, respostas atrasadas, cancelamento
da localização, menus do Painel e do Canvas e preservação de ações pendentes entre
abas. Os cenários `photoshop-*` no manifesto de aceitação usam componentes de
produção com portas determinísticas; essas capturas não comprovam o lançamento
do Adobe Photoshop nem substituem teste em uma instalação real.
