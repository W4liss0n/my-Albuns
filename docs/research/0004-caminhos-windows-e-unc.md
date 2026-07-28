---
status: current
document: technical-research
ticket: 01-plataforma-e-arquitetura
date: 2026-07-28
---

# Caminhos Windows e UNC no núcleo Rust

> Pesquisa técnica não normativa. Este documento registra evidências e recomendações substituíveis; decisões vigentes permanecem nos ADRs, na especificação e nos designs aprovados.

## Resumo executivo

O núcleo Rust deve representar caminhos com `Path`/`PathBuf` e `OsStr`/`OsString`, sem converter a identidade do caminho para `String`, aplicar `casefold` ou pressupor UTF-8. O caminho escolhido pelo usuário deve ser preservado para persistência e apresentação. Um caminho resolvido pelo sistema pode ser usado durante uma operação, mas não deve substituir automaticamente o valor salvo no Projeto.

Para o MyAlbuns, a solução enxuta é:

- usar as Known Folders do Windows, possivelmente por `directories::BaseDirs`, e acrescentar explicitamente a pasta `MyAlbuns`;
- aceitar caminhos absolutos de disco, UNC e suas formas verbatim;
- rejeitar caminhos relativos a uma unidade, como `C:foto.jpg`, namespaces de dispositivo, curingas, fluxos alternativos e componentes reservados;
- avaliar `same-file` somente como evidência auxiliar para arquivos ou diretórios que existem e podem ser abertos;
- considerar uma falha de comparação como identidade indeterminada, nunca como prova de que os arquivos são diferentes;
- deixar a política de `Same`, `Different` ou `Indeterminate` com o chamador;
- capturar um `RootBindingPlan` por tentativa e transmiti-lo aos processos participantes, sem cache global ou persistente;
- manter o Cache em armazenamento local, mesmo quando Projeto ou Mídias estejam em rede;
- restringir `dunce` às bordas de apresentação ou à interoperabilidade com programas que não aceitem caminhos verbatim;
- não usar canonicalização como identidade persistente.

## Modelo de caminhos do Rust

[`Path`](https://doc.rust-lang.org/std/path/struct.Path.html) é uma visão emprestada de um caminho, enquanto [`PathBuf`](https://doc.rust-lang.org/std/path/struct.PathBuf.html) é sua forma possuída. Ambos operam sobre a representação nativa do sistema por meio de `OsStr`/`OsString`. Converter com `to_string_lossy` é adequado para uma mensagem de interface, mas pode substituir dados não Unicode pelo caractere de substituição; portanto, não serve para persistência, comparação ou abertura do arquivo.

No Windows, a presença de um prefixo não garante que o caminho seja absoluto. A documentação de `Path::is_absolute` diferencia `C:\fotos`, que é absoluto, de `C:fotos`, que depende do diretório corrente da unidade C. A validação de entrada deve usar `is_absolute` e examinar os componentes, em vez de testar apenas a existência de uma letra de unidade.

[`std::path::Prefix`](https://doc.rust-lang.org/std/path/enum.Prefix.html) reconhece as formas relevantes:

| Variante | Exemplo | Tratamento recomendado |
| --- | --- | --- |
| `Disk` | `C:` em `C:\fotos` | Aceitar somente quando o caminho completo for absoluto. |
| `UNC` | `\\servidor\compartilhamento` | Aceitar como raiz de rede. Servidor e compartilhamento fazem parte da raiz. |
| `VerbatimDisk` | `\\?\C:` | Aceitar internamente; evitar simplificação antes de uma operação de arquivo. |
| `VerbatimUNC` | `\\?\UNC\servidor\compartilhamento` | Aceitar internamente, inclusive para caminhos longos. |
| `Verbatim` | `\\?\...` fora das formas de disco ou UNC | Rejeitar no MVP. Somente `VerbatimDisk` e `VerbatimUNC` entram no contrato. |
| `DeviceNS` | `\\.\COM42` | Rejeitar para Projeto, Mídia e destino de Exportação. Não é um caminho comum de arquivo. |

Essa classificação também impede que nomes de dispositivos, pipes ou outros objetos do namespace Win32 entrem acidentalmente no fluxo de Mídias.

## Pastas conhecidas do aplicativo

O crate [`directories`](https://docs.rs/directories/latest/directories/) usa o sistema de Known Folders no Windows. Para preservar exatamente a estrutura já aprovada pelo produto, [`BaseDirs`](https://docs.rs/directories/latest/directories/struct.BaseDirs.html) é o candidato adequado: ele localiza as raízes conhecidas, e o módulo acrescenta o único componente fixo `MyAlbuns`.

No Windows, a distribuição pretendida é:

- `config_dir()\MyAlbuns` em `%APPDATA%\MyAlbuns`, para `settings.json` e `Layouts`;
- `data_local_dir()\MyAlbuns` em `%LOCALAPPDATA%\MyAlbuns`, para `Cache`, `Recovery`, `State` e `Logs`.

O componente `MyAlbuns` é controlado pelo aplicativo, não vem de texto do usuário. Não se usa `ProjectDirs`, pois seus sufixos convencionais (`config`, `cache` e `data`) criariam uma árvore diferente da definida em [Armazenamento local e Cache](../design/0010-armazenamento-local-e-cache.md). `state_dir()` e `runtime_dir()` não fornecem uma pasta no Windows nessa API; portanto, seu retorno não deve ser pressuposto.

Pastas de Projeto, Mídias e Exportação continuam sendo escolhidas pelo usuário. `BaseDirs` não substitui essa escolha.

## Equivalência de arquivos existentes

Comparar strings de caminho não responde se duas grafias apontam para o mesmo arquivo. Letras de unidade, UNC, links, pontos de montagem, variações de caixa e aliases de servidor podem produzir nomes diferentes para o mesmo objeto.

O crate [`same-file`](https://docs.rs/same-file/latest/same_file/) oferece uma comparação multiplataforma baseada em handles. Sua função [`is_same_file`](https://docs.rs/same-file/latest/same_file/fn.is_same_file.html):

- exige que os dois caminhos possam ser abertos;
- retorna erro para caminho ausente ou sem permissão;
- documenta a possibilidade de falso positivo em algumas plataformas;
- no Windows, compara uma combinação de identificador, serial do volume e tamanho, mas documenta limitações que permitem falso positivo.

Consequentemente, `same-file` isoladamente é uma evidência candidata, não a prova autoritativa exigida para abrir uma segunda sessão de Projeto. A implementação Windows deve avaliar handles e as informações de identidade disponíveis — preferindo `GetFileInformationByHandleEx` com `FileIdInfo` quando suportado — atrás de uma interface com três resultados: `Same`, `Different` e `Indeterminate`. Uma evidência insuficiente ou um erro de rede, permissão ou ausência produz `Indeterminate`, nunca `Different`.

Essa comparação é apropriada para impedir que origem e destino existentes sejam o mesmo arquivo e para reconhecer aliases durante uma operação. Ela não deve ser gravada como identidade permanente do Projeto, e o bloqueio do arquivo continua sendo a proteção final contra duas sessões editáveis.

## `FILE_ID_INFO` e limites em rede

A estrutura Win32 [`FILE_ID_INFO`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info) contém o serial do volume e um identificador de arquivo de 128 bits. A Microsoft afirma que a combinação identifica unicamente um arquivo em um único computador e permite comparar dois handles abertos.

Essa garantia não transforma o par em um identificador global ou durável. Ela não promete estabilidade após cópia, restauração, troca de servidor, reconexão de compartilhamento ou migração de volume. Por isso, um ID de arquivo pode auxiliar uma comparação durante a sessão, mas não deve substituir:

- o identificador próprio do Projeto;
- o caminho vinculado da Mídia;
- o fluxo de Relink.

As limitações são mais importantes em rede. A documentação de [`GetFileInformationByHandle`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle) informa que, conforme o sistema e o servidor remoto, a chamada pode falhar ou retornar informação parcial; SMB 3.0 é suportado, mas isso não elimina diferenças de provedor e permissão.

Também não se deve pressupor que obter o caminho final seja barato ou sempre possível. [`GetFinalPathNameByHandleW`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfinalpathnamebyhandlew) explica que SMB não oferece diretamente consultas de caminho normalizado. O Windows pode consultar cada componente, e a falta de permissão em um único diretório pode causar `ERROR_ACCESS_DENIED`.

Na prática:

- indisponibilidade e permissão devem permanecer erros operacionais, não conclusões de identidade;
- o aplicativo não deve varrer ou canonicalizar toda a biblioteca antecipadamente;
- resultados de existência, metadata e identidade não devem ser mantidos como verdade durável para uma raiz SMB;
- desconexão durante Importação, Cache ou Exportação deve falhar de modo recuperável e preservar o Projeto.

## Unidade mapeada versus UNC

Uma unidade mapeada, como `Z:\Fotos`, pode representar `\\servidor\Fotos`, mas a letra é específica de usuário e sessão. A documentação da Microsoft sobre [serviços e unidades redirecionadas](https://learn.microsoft.com/en-us/windows/win32/services/services-and-redirected-drives) alerta que letras não são globais e podem não existir em outro contexto de logon. O mesmo ocorre entre processos elevados e não elevados em certos cenários.

[`WNetGetUniversalNameW`](https://learn.microsoft.com/en-us/windows/win32/api/winnetwk/nf-winnetwk-wnetgetuniversalnamew) pode converter um caminho baseado em unidade de rede para UNC, inclusive quando o item final ainda não existe. A chamada, porém, pode falhar por ausência da conexão, indisponibilidade da rede ou falta de suporte do provedor.

A recomendação é:

1. preservar no Projeto o caminho escolhido pelo usuário;
2. aceitar tanto unidade mapeada quanto UNC;
3. resolver unidade mapeada para UNC apenas como contexto operacional ou diagnóstico;
4. usar comparação por handle quando ambos os objetos existirem;
5. nunca concluir equivalência apenas porque duas strings UNC parecem iguais ou diferentes.

Isso também evita reescrever silenciosamente os vínculos do usuário quando uma letra de unidade é válida em sua sessão.

## Caminhos longos e formas verbatim

A documentação da Microsoft sobre [limites de comprimento](https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation) distingue o limite legado `MAX_PATH` das formas estendidas. Caminhos locais podem usar `\\?\C:\...`, e caminhos de rede podem usar `\\?\UNC\servidor\compartilhamento\...`.

Há três cuidados independentes:

- a aplicação deve declarar `longPathAware` no manifesto para APIs Win32 compatíveis, e o comportamento ampliado também depende da configuração do sistema;
- caminhos verbatim exigem separadores `\` e reduzem o processamento automático de `.` e `..`;
- o sistema de arquivos e o Shell não têm suporte idêntico, portanto um caminho aceito pelo núcleo pode falhar em um programa externo.

O opt-in `longPathAware` e a sintaxe verbatim são mecanismos relacionados, mas não equivalentes; um não autoriza simplificar ou remover o outro. O MyAlbuns deve usar APIs Unicode e ser testado com caminhos acima de 260 caracteres, mas não deve prometer interoperabilidade irrestrita com Photoshop, Explorer ou outro aplicativo externo. Nessa borda, uma falha deve ser apresentada claramente.

## Riscos da canonicalização

[`std::fs::canonicalize`](https://doc.rust-lang.org/std/fs/fn.canonicalize.html) resolve componentes intermediários e links simbólicos. No Windows, atualmente usa `CreateFile` e `GetFinalPathNameByHandle`, retorna sintaxe de comprimento estendido e exige que o caminho exista. A própria documentação alerta que a forma resultante pode ser incompatível com outros aplicativos.

Usá-la como normalização universal introduziria vários problemas:

- falha para arquivo ausente, offline ou sem permissão;
- pode transformar o caminho apresentado e escolhido pelo usuário;
- resolve links e pontos de montagem, alterando a grafia conforme o estado do sistema;
- em SMB, pode exigir acesso a todos os componentes;
- cria uma janela entre verificar e usar o arquivo;
- não produz uma identidade durável.

O crate [`dunce`](https://docs.rs/dunce/latest/dunce/) oferece uma canonicalização que tenta devolver a forma mais compatível e [`dunce::simplified`](https://docs.rs/dunce/latest/dunce/fn.simplified.html) remove a forma verbatim quando isso é inequívoco. Ele deve ser considerado somente para apresentação ou para passar um caminho a um programa sem suporte a verbatim. Não deve ser aplicado ao valor persistido nem antes de operações internas de arquivo.

## Recomendação enxuta

Um módulo Rust compartilhado, e não um novo processo, deve centralizar essas regras:

- `AppPaths::discover()` obtém as raízes roaming e local com Known Folders, possivelmente por `BaseDirs`, e acrescenta `MyAlbuns`;
- `resolve(path, purpose)` valida prefixo, forma absoluta e, depois da abertura, o tipo de objeto exigido;
- `equivalent_existing(a, b)` retorna `Same`, `Different` ou `Indeterminate`;
- um `RootBindingPlan` imutável por tentativa registra somente tipo, raiz lógica, binding operacional e representação nativa escolhida; cada processo participante cria um contexto efêmero a partir dele;
- capacidades concretas, como substituição atômica, são verificadas pelo consumidor no uso e não entram como fatos genéricos do plano;
- existência, metadata e identidade de arquivos individuais não são reutilizadas como verdade permanente;
- o staging de Exportação deve ser criado dentro da própria pasta de Destino, sem prometer atomicidade quando o sistema de arquivos ou servidor não a oferecer;
- o Cache permanece local independentemente da localização de Projeto e Mídias.

Chamadas que possam alcançar UNC são executadas fora da thread da interface. Derivações relativas sob uma raiz validada aceitam apenas componentes comuns: rejeitam nova raiz, `..`, nomes de dispositivo, fluxo alternativo (`:`) e qualquer forma que escape do Destino. Reparse points exigem validação no momento de uso; uma checagem textual anterior não substitui a abertura real.

Ficam deliberadamente fora da primeira solução:

- daemon ou monitor próprio de rede;
- tabela persistente de aliases entre unidade e UNC;
- canonicalização de toda a biblioteca;
- identidade global baseada em `FILE_ID_INFO`;
- TTL ou banco persistente de metadata de rede;
- tentativa de distinguir perfeitamente todos os erros produzidos por servidores SMB.

## Relação com as decisões vigentes

- O [ADR 0001](../adr/0001-vincular-arquivos-externos.md) mantém mídias como Arquivos vinculados e separa ausência confirmada de indisponibilidade.
- O [ADR 0002](../adr/0002-identificar-copias-externas.md) governa aliases, movimentações e Cópias externas de Projeto.
- O [ADR 0006](../adr/0006-publicar-exportacao-com-transacao-limitada.md) exige preparação dentro do Destino e limita a garantia da Publicação.
- O [ADR 0007](../adr/0007-tratar-caminhos-windows-e-identidade-fisica.md) contém a decisão arquitetural derivada desta pesquisa.
- [Armazenamento local e Cache](../design/0010-armazenamento-local-e-cache.md) define a árvore exata sob `%APPDATA%` e `%LOCALAPPDATA%`.
- [Resolução e política de caminhos](../design/0011-resolucao-e-politica-de-caminhos.md) define a interface e os cenários normativos; crates mencionadas aqui continuam substituíveis.

## Matriz mínima de testes

A implementação deve cobrir:

- caminho local absoluto e não ASCII;
- `.\relativo`, `..\relativo` e `\relativo` rejeitados;
- `C:relativo` rejeitado;
- UNC, VerbatimDisk e VerbatimUNC;
- `Verbatim` genérico rejeitado;
- namespace de dispositivo rejeitado;
- curinga, fluxo alternativo e componente reservado rejeitados;
- arquivo fornecido onde se esperava diretório e o inverso;
- caminho acima de 260 caracteres;
- unidade mapeada e seu alias UNC;
- o mesmo `RootBindingPlan` usado pelo host e pelo Processador;
- novo plano depois de nova tentativa ou retomada;
- dois caminhos existentes para o mesmo arquivo;
- arquivo ausente, sem permissão e compartilhamento offline;
- desconexão de rede no meio de leitura;
- Cache local com Mídia remota;
- staging dentro da própria pasta de Destino;
- apresentação compatível sem alterar o caminho persistido.

A serialização reversível de um `OsString` Windows que não seja Unicode válido precisa ser decidida junto do formato do documento de Projeto. Até essa decisão, converter com perda para UTF-8 não é uma implementação aceitável.

## Fontes primárias

- [Rust `Path`](https://doc.rust-lang.org/std/path/struct.Path.html)
- [Rust `PathBuf`](https://doc.rust-lang.org/std/path/struct.PathBuf.html)
- [Rust `Prefix`](https://doc.rust-lang.org/std/path/enum.Prefix.html)
- [Rust `std::fs::canonicalize`](https://doc.rust-lang.org/std/fs/fn.canonicalize.html)
- [`directories` no docs.rs](https://docs.rs/directories/latest/directories/)
- [`same-file` no docs.rs](https://docs.rs/same-file/latest/same_file/)
- [`dunce` no docs.rs](https://docs.rs/dunce/latest/dunce/)
- [Microsoft: Naming Files, Paths, and Namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file)
- [Microsoft: Maximum Path Length Limitation](https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation)
- [Microsoft: `WNetGetUniversalNameW`](https://learn.microsoft.com/en-us/windows/win32/api/winnetwk/nf-winnetwk-wnetgetuniversalnamew)
- [Microsoft: `GetFinalPathNameByHandleW`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfinalpathnamebyhandlew)
- [Microsoft: `FILE_ID_INFO`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info)
- [Microsoft: `GetFileInformationByHandleEx`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandleex)
- [Microsoft: `GetFileInformationByHandle`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle)
- [Microsoft: Services and Redirected Drives](https://learn.microsoft.com/en-us/windows/win32/services/services-and-redirected-drives)
