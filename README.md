# orangeEncrypt

Português: [README-pt_BR.md](README-pt_BR.md)

Encrypt files and folders on your computer, with a password, in the open OpenPGP format.

There is no account, no server, and no proprietary format. If orangeEncrypt disappears, the file can still be recovered with [GnuPG](https://gnupg.org/).

Website: [apps.orange8.net](https://apps.orange8.net)

## What it does

Drop a file, enter a password, and get a `.gpg`.

| Input | Result |
| --- | --- |
| One file | `documento.pdf.gpg` |
| One folder | `Documentos.tar.gpg` |
| Several files or folders | `Archive.tar.gpg` |

A single file does not go through TAR. Multiple entries are packed into an uncompressed TAR and only then encrypted. Decryption recognizes the TAR from its contents.

## Recover without the app

A single file:

```bash
gpg --output documento.pdf --decrypt documento.pdf.gpg
```

A package with several files:

```bash
gpg --output arquivo.tar --decrypt arquivo.tar.gpg
tar -xf arquivo.tar
```

A wrong password does not reveal the contents. A forgotten password cannot be recovered from the `.gpg`.

## How it is protected

Sequoia PGP writes a binary OpenPGP message:

- AES-256
- session key in an SKESK
- password derived with S2K
- integrity with SEIPD v1 (RFC 4880 MDC)

There is no ASCII armor, no public key, and no AEAD. Writing does not compress. Reading accepts the compression GnuPG uses by default.

The original is left unchanged. The partial destination is named `*.gpg.partial` and is renamed only at the end. Cancel deletes the partial.

The password travels from the interface to the local process and is wiped from memory. It is not passed as a process argument, written to a temporary file, or stored in `localStorage`.

The backend is Sequoia’s RustCrypto, which Sequoia marks as experimental and not constant-time. It was chosen so the app does not depend on Nettle or OpenSSL being installed on the system.

## Languages

The language follows the system: Portuguese, English, Spanish, German, French, Japanese, and Chinese. A locale without a translation uses English.

## Development

You need Node.js, Rust, and the system build tools. GnuPG is used only by the interoperability tests.

```bash
npm install
npm run tauri dev
```

To force a language for this session:

```bash
VITE_LOCALE=en npm run tauri dev
```

Tests:

```bash
npm test
cargo test --manifest-path src-tauri/Cargo.toml
```

Production build:

```bash
npm run tauri build
```

GitHub Actions also builds the installers on every push to `main` and publishes a release when the tag starts with `v`. The targets are macOS Apple Silicon, Windows x64 and ARM, and Linux x64 and ARM.

## License

GNU General Public License v3.0. See [LICENSE](LICENSE).
