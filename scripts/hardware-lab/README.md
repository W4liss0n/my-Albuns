# Laboratório de hardware fraco

Mede o instalador do MyAlbuns numa VM Hyper-V que imita um computador mais
fraco: poucos núcleos lentos, pouca memória e disco lento. Cada medição começa
de um Windows limpo, instala o app sem perguntas, reinicia a VM para esvaziar
o cache de arquivos e abre um projeto. Em seguida rola e aplica zoom no canvas
pelo DevTools enquanto registra quadros, tarefas longas, memória, CPU e disco.
Ninguém precisa clicar durante a medição, e a VM não toma o mouse nem a tela
do computador.

## O que a VM imita e o que não imita

| Condição | Fidelidade |
|---|---|
| Memória, número de núcleos, Windows limpo | boa |
| Núcleo lento | aproximada: limite de CPU por vCPU |
| Disco lento | boa: limite de IOPS do Hyper-V |
| GPU fraca | **não imita**: a VM usa uma fatia da GPU deste computador |

O app recusa WebGL2 feito por software, por isso a VM precisa de partição de
GPU (GPU-P). A Intel UHD fica mais perto de uma GPU integrada antiga e é o
padrão. A VM não tem rede: projetos com fotos em pasta de rede não servem, e o
WebView2 fica na versão que veio na ISO.

## Montagem, uma vez

Requisitos: Hyper-V ativo, cerca de 40 GB livres em `D:`, Node no PATH e uma
ISO oficial do Windows 11, de <https://www.microsoft.com/software-download/windows11>
(ou do Windows 10, se for o caso do usuário). Sem ativação o Windows funciona,
só mostra a marca-d'água.

Num PowerShell aberto **como administrador**:

```powershell
& 'scripts\hardware-lab\New-HardwareLabVm.ps1' -IsoPath 'D:\ISOs\Win11_Portuguese_x64.iso' -GrantHyperVAdministrators
```

O script aplica a imagem, copia o driver da GPU, cria a VM `myalbuns-lab` em
`D:\HardwareLab` e espera o primeiro login automático (10 a 30 minutos). No fim,
mostra a placa de vídeo vista dentro da VM. `-GrantHyperVAdministrators` põe
seu usuário no grupo "Administradores do Hyper-V": depois de sair e entrar de
novo no Windows, as medições rodam sem elevação.

Depois de atualizar o driver da GPU do computador, recrie a VM com `-Force`.

## Álbum de teste

`new-mock-album.py` cria um álbum de casamento falso com o peso de um real:

- 300 JPEGs de 24 a 30 MP, com 8 a 13 MB cada (cerca de 2,9 GB no total), com EXIF de câmera;
- parte das fotos de retrato vem deitada e fica em pé pela orientação do EXIF;
- três pastas, uma delas com acento (`Cerimônia`);
- um projeto com 40 lâminas e 112 fotos já diagramadas.

O gerador precisa de Pillow e numpy.

```powershell
python scripts\hardware-lab\new-mock-album.py --output D:\HardwareLab\fixtures\album-mock
```

## Medição

```powershell
& 'scripts\hardware-lab\Invoke-HardwareLabRun.ps1' -Installer 'target\release\bundle\nsis\MyAlbuns_0.1.0_x64-setup.exe' -ProjectFile 'D:\Teste\Album\album.myalbuns' -HardwareProfile fraco -Repeat 2
```

- O projeto e a pasta dele (ou as pastas de `-MediaFolder`) são copiados para o
  mesmo caminho dentro da VM, por isso precisam estar em `C:` ou `D:`. O que
  está em `D:` fica no disco de dados da VM e só é copiado de novo quando muda.
- `-Repeat 2` abre o projeto duas vezes no mesmo boot: a primeira abertura é
  fria, a segunda é morna, com o Cache do app já pronto.
- `-KeepRunning` deixa a VM ligada para olhar a tela no Gerenciador do Hyper-V.

O resultado fica em `D:\HardwareLab\runs\<data>-<perfil>\`:

- `summary.md`: tempos da abertura, quadros por fase, tarefas longas e picos de
  memória, CPU e disco;
- `scenario-N.json`: dados brutos do DevTools; `screenshot-N.png`: a tela no fim;
- `counters.csv`: uma amostra por segundo do convidado; `logs/`: logs do app.

## Perfis

Os perfis ficam em `profiles.json` e podem ser ajustados às especificações do
usuário. O limite de IOPS do Hyper-V conta blocos de 8 KB, então também limita
a leitura sequencial: 5000 IOPS dão cerca de 40 MB/s. O limite de CPU vale por
vCPU, e 40% de um núcleo moderno equivale aproximadamente a um núcleo de
notebook antigo.

## Privacidade

As fotos copiadas ficam nos discos da VM em `D:\HardwareLab`, fora do Git. Não
use pastas de clientes sem combinar antes, e apague a VM quando ela não for
mais necessária.
