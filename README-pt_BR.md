# orangeEncrypt

English: [README.md](README.md)

Criptografe arquivos e pastas no seu computador, com uma senha, no formato aberto OpenPGP.

Não há conta, servidor nem formato proprietário. Se o orangeEncrypt deixar de existir, o arquivo continua recuperável com o [GnuPG](https://gnupg.org/).

Site: [apps.orange8.net](https://apps.orange8.net)

## O que ele faz

Arraste um arquivo, informe a senha e receba um `.gpg`.

| Entrada | Resultado |
| --- | --- |
| Um arquivo | `documento.pdf.gpg` |
| Uma pasta | `Documentos.tar.gpg` |
| Vários arquivos ou pastas | `Archive.tar.gpg` |

Um único arquivo não passa por TAR. Várias entradas são empacotadas em TAR, sem compressão, e só então criptografadas. A descriptografia reconhece o TAR pelo conteúdo.

## Recuperar sem o aplicativo

Arquivo único:

```bash
gpg --output documento.pdf --decrypt documento.pdf.gpg
```

Pacote com vários arquivos:

```bash
gpg --output arquivo.tar --decrypt arquivo.tar.gpg
tar -xf arquivo.tar
```

A senha errada não revela o conteúdo. Quem esquecer a senha não a recupera a partir do `.gpg`.

## Como é protegido

A Sequoia PGP escreve uma mensagem OpenPGP binária:

- AES-256
- chave de sessão em SKESK
- senha derivada por S2K
- integridade por SEIPD v1 (MDC da RFC 4880)

Não há ASCII armor, chave pública nem AEAD. A gravação não comprime. A leitura aceita a compressão que o GnuPG usa por padrão.

O original não é alterado. O destino parcial se chama `*.gpg.partial` e só é renomeado no fim. Cancelar apaga o parcial.

A senha vai da interface para o processo local e é zerada em memória. Ela não entra em argumento de processo, arquivo temporário ou `localStorage`.

O backend é o RustCrypto da Sequoia, marcado por ela como experimental e sem garantia de tempo constante. Ele foi escolhido para o aplicativo não depender de Nettle ou OpenSSL instalados no sistema.

## Idiomas

O idioma segue o sistema: português, inglês, espanhol, alemão, francês, japonês e chinês. Um locale sem tradução usa inglês.

## Desenvolvimento

É preciso Node.js, Rust e as ferramentas de compilação do sistema. O GnuPG entra só nos testes de interoperabilidade.

```bash
npm install
npm run tauri dev
```

Para forçar um idioma nesta sessão:

```bash
VITE_LOCALE=en npm run tauri dev
```

Testes:

```bash
npm test
cargo test --manifest-path src-tauri/Cargo.toml
```

Build de produção:

```bash
npm run tauri build
```

O GitHub Actions também gera os instaladores em cada push na `main` e publica uma release quando a tag começa com `v`. Os alvos são macOS Apple Silicon, Windows x64 e ARM, e Linux x64 e ARM.

## Licença

GNU General Public License v3.0. Veja [LICENSE](LICENSE).
