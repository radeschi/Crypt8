# orangeEncrypt

Criptografia de arquivos simples e multiplataforma, com OpenPGP.

Um aplicativo de mesa pequeno para criptografar arquivos no seu computador. Não há conta, servidor nem envio do arquivo como parte da criptografia. O resultado é um `.gpg` OpenPGP comum.

English: [README.md](README.md)

[![Licença: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-orange)](LICENSE)
[![Release no GitHub](https://img.shields.io/github/v/release/radeschi/orangeEncrypt)](https://github.com/radeschi/orangeEncrypt/releases)
[![Build](https://img.shields.io/github/actions/workflow/status/radeschi/orangeEncrypt/build.yml?branch=main)](https://github.com/radeschi/orangeEncrypt/actions/workflows/build.yml)

Site: [apps.orange8.net](https://apps.orange8.net)

![orangeEncrypt demo](orangeEncrypt-demo.gif)

## Precisa proteger um arquivo antes de enviar ou guardar?

Um atalho comum é colocar o arquivo num ZIP e definir uma senha. Isso funciona como embalagem: ZIP é um formato de arquivamento e compressão que também pode levar uma senha.

O orangeEncrypt faz um trabalho mais estreito. Quando o objetivo é proteger um arquivo, a criptografia é a própria operação. Não é preciso criar uma conta, enviar o arquivo a um serviço nem embrulhá-lo num arquivo compactado só para trancá-lo.

## Por que o orangeEncrypt?

O aplicativo fica na sua máquina. A criptografia não envia o arquivo e não pede conta nem servidor.

O código é aberto. O arquivo criptografado é OpenPGP comum, então ainda pode ser aberto com o [GnuPG](https://gnupg.org/) se este aplicativo deixar de existir.

ZIP é ótimo para empacotar arquivos. O orangeEncrypt foi feito para outra tarefa: criptografá-los.

ZIP com senha:

`arquivo → pacote → senha`

orangeEncrypt:

`arquivo → criptografia → arquivo criptografado`

## Como funciona

1. Escolha um arquivo ou solte-o na janela.
2. Informe a senha e confirme.
3. Criptografe.
4. Receba um `.gpg` ao lado do original. O original não é modificado.
5. Descriptografe depois com a mesma senha.

| Entrada | Resultado |
| --- | --- |
| Um arquivo | `documento.pdf.gpg` |
| Uma pasta | `Documentos.tar.gpg` |
| Vários arquivos ou pastas | `Archive.tar.gpg` |

Um único arquivo é criptografado como ele mesmo. Várias entradas são empacotadas num TAR sem compressão e só então criptografadas. A descriptografia reconhece o TAR pelo conteúdo, não só pelo nome.

## Recursos

- Criptografia e descriptografia locais de arquivos e pastas
- Proteção por senha, com confirmação da senha antes de criptografar
- Saída OpenPGP (`.gpg`), legível pelo GnuPG
- Janela pequena: solte um arquivo ou escolha um
- Progresso, estimativa de tempo restante e cancelamento
- Cancelar apaga o arquivo parcial. O original permanece como estava
- Descriptografar e, em seguida, abrir o resultado ou salvá-lo em outro lugar
- O idioma segue o sistema: português, inglês, espanhol, alemão, francês, japonês e chinês. Os demais usam inglês
- Código aberto, GPL-3.0
- Verificação de atualização assinada a partir das releases do GitHub, com conferência manual em Sobre
- Instaladores para macOS (Apple Silicon e Intel), Windows (x64 e ARM) e Linux (x64 e ARM)

## orangeEncrypt e Encrypto

Os dois são aplicativos de mesa para proteger arquivos com senha, no computador onde o arquivo já está. A diferença destacada aqui é que o orangeEncrypt publica o código-fonte.

| | orangeEncrypt | Encrypto |
| --- | --- | --- |
| Aplicativo de mesa | ✓ | ✓ |
| Criptografia local | ✓ | ✓ |
| Proteção por senha | ✓ | ✓ |
| macOS | ✓ | ✓ |
| Windows | ✓ | ✓ |
| Código aberto | ✓ | — |
| Código disponível para leitura | ✓ | — |

O orangeEncrypt é uma alternativa de código aberto para quem quer um fluxo simples de criptografia no desktop e poder ler o código.

## Por que não usar só um ZIP com senha?

Um ZIP com senha pode ser útil, principalmente quando você também precisa arquivar e comprimir. O orangeEncrypt mira outro fluxo: quando o objetivo principal é proteger um arquivo, a criptografia é a operação principal.

Um único arquivo continua sendo um único `.gpg`. Ele não é comprimido e não entra num ZIP.

## Criptografia

A [Sequoia PGP](https://sequoia-pgp.org/) grava uma mensagem OpenPGP binária:

- AES-256
- chave de sessão em SKESK
- senha derivada por S2K
- integridade por SEIPD v1 (MDC da RFC 4880)

Não há ASCII armor, chave pública nem AEAD. A gravação não comprime. A leitura aceita a compressão que o GnuPG usa por padrão.

O arquivo original não muda. A saída vai para `*.gpg.partial` e só é renomeada no fim. Cancelar apaga esse parcial.

A senha vai da janela para o processo local e é zerada na memória. Ela não entra em argumento de processo, arquivo temporário ou `localStorage`.

O backend criptográfico é o RustCrypto da Sequoia. A Sequoia marca esse backend como experimental e sem garantia de tempo constante. Ele foi escolhido para o aplicativo não depender de Nettle ou OpenSSL instalados no sistema.

A senha errada não revela o conteúdo. Quem esquece a senha não a recupera a partir do `.gpg`.

Sem o aplicativo, o GnuPG ainda abre o arquivo.

Um arquivo:

```bash
gpg --output documento.pdf --decrypt documento.pdf.gpg
```

Um pacote com vários arquivos:

```bash
gpg --output arquivo.tar --decrypt arquivo.tar.gpg
tar -xf arquivo.tar
```

## Privacidade

A criptografia acontece no computador em que você abre o aplicativo. Esse fluxo não envia o arquivo a um servidor e não exige conta.

Esse é o alcance da afirmação. O aplicativo não promete anonimato, e um `.gpg` só é tão privado quanto a senha e os lugares para onde você depois o copiar.

## Instalação

Baixe um instalador nas [releases](https://github.com/radeschi/orangeEncrypt/releases).

O GitHub Actions gera estes alvos:

- macOS Apple Silicon e Intel
- Windows x64 e ARM
- Linux x64 e ARM

Os builds de macOS publicados pelo GitHub não são notarizados. O macOS pode pedir que você autorize o aplicativo em Privacidade e Segurança antes de abrir.

As atualizações, numa build que já as inclua, são conferidas na release mais recente do GitHub e rejeitadas se a assinatura não corresponder.

## Uso

1. Abra o orangeEncrypt.
2. Solte um arquivo ou escolha um.
3. Informe a senha e confirme.
4. Clique em Criptografar.
5. O `.gpg` é criado localmente. O original permanece no lugar.

Para descriptografar, solte um `.gpg`, confirme que deseja descriptografar e informe a senha. Num arquivo comum, dá para abrir ou salvar uma cópia. Em vários arquivos empacotados como TAR, abrir mostra a pasta e salvar pede um diretório.

## Código aberto

O orangeEncrypt usa a GNU General Public License v3.0. Veja [LICENSE](LICENSE).

## Estado do projeto

A versão nesta árvore de código é 1.0.5. O projeto está em desenvolvimento ativo: o fluxo de criptografar e descriptografar já pode ser usado, e a interface e as ferramentas de release ainda estão sendo refinadas.

## Contribuir

Issues e pull requests são bem-vindos. Use as [issues](https://github.com/radeschi/orangeEncrypt/issues) para bugs e ideias. Ainda não há um guia de contribuição separado.

O desenvolvimento precisa de Node.js, Rust e das ferramentas de compilação do sistema. O GnuPG entra só nos testes de interoperabilidade.

```bash
npm install
npm run tauri dev
```

Forçar um idioma nesta sessão:

```bash
VITE_LOCALE=pt-BR npm run tauri dev
```

Testes e build de produção local:

```bash
npm test
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build
```

## Links

- [Repositório](https://github.com/radeschi/orangeEncrypt)
- [Releases](https://github.com/radeschi/orangeEncrypt/releases)
- [Issues](https://github.com/radeschi/orangeEncrypt/issues)
- [apps.orange8.net](https://apps.orange8.net)
