use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sequoia_openpgp as openpgp;
use sequoia_openpgp::crypto::Password;
use sequoia_openpgp::packet::Tag;
use sequoia_openpgp::parse::stream::{DecryptorBuilder, DecryptionHelper, VerificationHelper};
use sequoia_openpgp::parse::{PacketParser, PacketParserResult, Parse};
use sequoia_openpgp::Packet;
use sequoia_openpgp::policy::StandardPolicy;
use sequoia_openpgp::armor::Kind as ArmorKind;
use sequoia_openpgp::serialize::stream::{Armorer, Encryptor, LiteralWriter, Message};
use sequoia_openpgp::types::{DataFormat, SymmetricAlgorithm};

use crate::filesystem::{self, conflicts_with_input, literal_filename, partial_output};
use crate::progress::{ProgressEstimator, ProgressSnapshot, ProgressStatus};

const CHUNK_SIZE: usize = 64 * 1024;

/// Limite da mensagem original, em caracteres Unicode. O bloco OpenPGP pode ser maior.
pub const MAX_TEXT_MESSAGE_LENGTH: usize = 2000;

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("arquivo não encontrado")]
    NotFound,
    #[error("sem permissão para ler o arquivo")]
    PermissionDenied,
    #[error("o caminho não é um arquivo")]
    NotAFile,
    #[error("o destino já existe")]
    DestinationExists,
    #[error("o destino é o próprio arquivo original")]
    RefusingToOverwriteInput,
    #[error("não foi possível ler o arquivo")]
    Read(#[source] io::Error),
    #[error("não foi possível criar o arquivo criptografado")]
    CreateOutput(#[source] io::Error),
    #[error("operação cancelada")]
    Cancelled,
    #[error("falha OpenPGP: {0}")]
    OpenPgp(String),
    #[error("caminho inseguro no pacote")]
    UnsafePath,
    #[error("link simbólico não é aceito")]
    Symlink,
    #[error("arquivo especial não é aceito")]
    SpecialFile,
    #[error("nenhum conteúdo informado")]
    EmptyText,
    #[error("senha vazia")]
    EmptyPassword,
    #[error("mensagem longa demais")]
    MessageTooLong,
    #[error("mensagem OpenPGP inválida")]
    InvalidMessage,
    #[error("senha incorreta")]
    WrongPassword,
}

pub struct EncryptOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub password: Password,
    pub overwrite: bool,
    pub cancel: Arc<AtomicBool>,
    pub progress_interval: Duration,
    pub on_progress: Box<dyn Fn(ProgressSnapshot) + Send>,
}

#[derive(Debug)]
pub struct EncryptOutcome {
    pub output: PathBuf,
    pub input_bytes: u64,
    pub output_bytes: u64,
}

struct PartialFile {
    path: PathBuf,
    armed: bool,
}

impl Drop for PartialFile {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Criptografa em streaming: lê blocos do arquivo original e escreve OpenPGP no destino.
///
/// O contêiner é o da Sequoia sem AEAD, isto é, SEIPD v1 (RFC 4880) com AES-256 e
/// SKESK/S2K escolhidos pela biblioteca. O arquivo original não é aberto para escrita.
pub fn encrypt_file(options: EncryptOptions) -> Result<EncryptOutcome, CryptoError> {
    let meta = fs::metadata(&options.input).map_err(map_read_error)?;
    if !meta.is_file() {
        return Err(CryptoError::NotAFile);
    }
    if conflicts_with_input(&options.input, &options.output)? {
        return Err(CryptoError::RefusingToOverwriteInput);
    }
    if options.output.exists() && !options.overwrite {
        return Err(CryptoError::DestinationExists);
    }

    let total = meta.len();
    let mut estimator = ProgressEstimator::new(total, options.progress_interval);
    (options.on_progress)(estimator.snapshot(ProgressStatus::Preparing));

    if options.cancel.load(Ordering::Relaxed) {
        return Err(CryptoError::Cancelled);
    }

    let partial_path = partial_output(&options.output);
    if partial_path.exists() {
        fs::remove_file(&partial_path).map_err(CryptoError::CreateOutput)?;
    }
    if let Some(parent) = partial_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(CryptoError::CreateOutput(io::Error::new(
                io::ErrorKind::NotFound,
                "pasta de destino inexistente",
            )));
        }
    }

    let mut guard = PartialFile {
        path: partial_path.clone(),
        armed: true,
    };
    let mut output_file = File::create(&partial_path).map_err(CryptoError::CreateOutput)?;
    let input_bytes = write_encrypted(&options, &mut output_file, &mut estimator, total)?;
    output_file.sync_all().map_err(CryptoError::CreateOutput)?;
    drop(output_file);

    replace_file(&partial_path, &options.output)?;
    guard.armed = false;

    let output_bytes = fs::metadata(&options.output)
        .map_err(CryptoError::CreateOutput)?
        .len();
    (options.on_progress)(estimator.push(input_bytes, ProgressStatus::Completed, true).unwrap());

    Ok(EncryptOutcome {
        output: options.output,
        input_bytes,
        output_bytes,
    })
}

/// Criptografa bytes produzidos por `produce` com os mesmos parâmetros de `encrypt_file`.
/// `encrypt_file` permanece o caminho de um único arquivo.
pub fn encrypt_generated(
    output: &Path,
    literal_name: &str,
    total_bytes: u64,
    password: &Password,
    overwrite: bool,
    cancel: &Arc<AtomicBool>,
    progress_interval: Duration,
    on_progress: &dyn Fn(ProgressSnapshot),
    produce: impl FnOnce(&mut dyn Write) -> Result<(), CryptoError>,
) -> Result<EncryptOutcome, CryptoError> {
    if output.exists() && !overwrite {
        return Err(CryptoError::DestinationExists);
    }
    let mut estimator = ProgressEstimator::new(total_bytes, progress_interval);
    on_progress(estimator.snapshot(ProgressStatus::Preparing));
    if cancel.load(Ordering::Relaxed) {
        return Err(CryptoError::Cancelled);
    }
    let partial_path = partial_output(output);
    if partial_path.exists() {
        fs::remove_file(&partial_path).map_err(CryptoError::CreateOutput)?;
    }
    let mut guard = PartialFile {
        path: partial_path.clone(),
        armed: true,
    };
    let mut output_file = File::create(&partial_path).map_err(CryptoError::CreateOutput)?;
    let mut writer = io::BufWriter::new(&mut output_file);
    let message = Message::new(&mut writer);
    let message = Encryptor::with_passwords(message, Some(password.clone()))
        .symmetric_algo(SymmetricAlgorithm::AES256)
        .build()
        .map_err(pgp)?;
    let mut literal = LiteralWriter::new(message)
        .format(DataFormat::Binary)
        .filename(literal_name)
        .map_err(pgp)?
        .build()
        .map_err(pgp)?;

    {
        let mut sink = ProgressWrite {
            inner: &mut literal,
            processed: 0,
            total: total_bytes,
            cancel,
            estimator: &mut estimator,
            on_progress,
        };
        produce(&mut sink)?;
        if cancel.load(Ordering::Relaxed) {
            return Err(CryptoError::Cancelled);
        }
    }

    on_progress(estimator.snapshot(ProgressStatus::Finalizing));
    literal.finalize().map_err(pgp)?;
    writer.flush().map_err(CryptoError::CreateOutput)?;
    drop(writer);
    output_file.sync_all().map_err(CryptoError::CreateOutput)?;
    replace_file(&partial_path, output)?;
    guard.armed = false;
    let output_bytes = fs::metadata(output).map_err(CryptoError::CreateOutput)?.len();
    let _ = estimator.push(total_bytes, ProgressStatus::Completed, true);
    Ok(EncryptOutcome {
        output: output.to_path_buf(),
        input_bytes: total_bytes,
        output_bytes,
    })
}

struct ProgressWrite<'a, W> {
    inner: &'a mut W,
    processed: u64,
    total: u64,
    cancel: &'a AtomicBool,
    estimator: &'a mut ProgressEstimator,
    on_progress: &'a dyn Fn(ProgressSnapshot),
}

impl<W: Write> Write for ProgressWrite<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }
        let written = self.inner.write(buf)?;
        self.processed = self.processed.saturating_add(written as u64);
        let shown = self.processed.min(self.total);
        if let Some(snapshot) = self.estimator.push(shown, ProgressStatus::Encrypting, false) {
            (self.on_progress)(snapshot);
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Lê só o início do texto claro, para reconhecer um TAR sem gravar o arquivo.
pub fn read_plaintext_prefix(path: &Path, password: &Password, limit: usize) -> Result<Vec<u8>, CryptoError> {
    password_opens(path, password)?;
    let input = File::open(path).map_err(map_read_error)?;
    let helper = PasswordHelper {
        password: password.clone(),
    };
    let policy = StandardPolicy::new();
    let mut decryptor = DecryptorBuilder::from_reader(input)
        .map_err(pgp)?
        .with_policy(&policy, None, helper)
        .map_err(pgp)?;
    let mut buf = vec![0u8; limit];
    let mut filled = 0;
    while filled < limit {
        match decryptor.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(err) => return Err(CryptoError::OpenPgp(err.to_string())),
        }
    }
    buf.truncate(filled);
    Ok(buf)
}

fn write_encrypted(
    options: &EncryptOptions,
    output_file: &mut File,
    estimator: &mut ProgressEstimator,
    total: u64,
) -> Result<u64, CryptoError> {
    let mut input = File::open(&options.input).map_err(map_read_error)?;
    let mut writer = io::BufWriter::new(output_file);
    let message = Message::new(&mut writer);
    let message = Encryptor::with_passwords(message, Some(options.password.clone()))
        .symmetric_algo(SymmetricAlgorithm::AES256)
        .build()
        .map_err(pgp)?;
    let mut literal = LiteralWriter::new(message)
        .format(DataFormat::Binary)
        .filename(literal_filename(&options.input))
        .map_err(pgp)?
        .build()
        .map_err(pgp)?;

    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut processed = 0u64;
    loop {
        if options.cancel.load(Ordering::Relaxed) {
            return Err(CryptoError::Cancelled);
        }
        let read = input.read(&mut buf).map_err(CryptoError::Read)?;
        if read == 0 {
            break;
        }
        literal
            .write_all(&buf[..read])
            .map_err(|err| CryptoError::OpenPgp(err.to_string()))?;
        processed = processed.saturating_add(read as u64);
        if let Some(snapshot) = estimator.push(processed, ProgressStatus::Encrypting, false) {
            (options.on_progress)(snapshot);
        }
    }

    if options.cancel.load(Ordering::Relaxed) {
        return Err(CryptoError::Cancelled);
    }

    (options.on_progress)(estimator.snapshot(ProgressStatus::Finalizing));
    literal.finalize().map_err(pgp)?;
    writer.flush().map_err(CryptoError::CreateOutput)?;
    debug_assert_eq!(processed, total);
    Ok(processed)
}

fn replace_file(from: &Path, to: &Path) -> Result<(), CryptoError> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            fs::remove_file(to).map_err(CryptoError::CreateOutput)?;
            fs::rename(from, to).map_err(CryptoError::CreateOutput)
        }
        Err(err) => Err(CryptoError::CreateOutput(err)),
    }
}

fn map_read_error(err: io::Error) -> CryptoError {
    match err.kind() {
        io::ErrorKind::NotFound => CryptoError::NotFound,
        io::ErrorKind::PermissionDenied => CryptoError::PermissionDenied,
        _ => CryptoError::Read(err),
    }
}

fn pgp(err: impl std::fmt::Display) -> CryptoError {
    CryptoError::OpenPgp(format!("{err:#}"))
}

struct PasswordHelper {
    password: Password,
}

impl VerificationHelper for PasswordHelper {
    fn get_certs(
        &mut self,
        _ids: &[openpgp::KeyHandle],
    ) -> openpgp::Result<Vec<openpgp::Cert>> {
        Ok(Vec::new())
    }

    fn check(&mut self, _structure: openpgp::parse::stream::MessageStructure) -> openpgp::Result<()> {
        Ok(())
    }
}

impl DecryptionHelper for PasswordHelper {
    fn decrypt(
        &mut self,
        _pkesks: &[openpgp::packet::PKESK],
        skesks: &[openpgp::packet::SKESK],
        _sym_algo: Option<SymmetricAlgorithm>,
        decrypt: &mut dyn FnMut(Option<SymmetricAlgorithm>, &openpgp::crypto::SessionKey) -> bool,
    ) -> openpgp::Result<Option<openpgp::Cert>> {
        for skesk in skesks {
            if skesk
                .decrypt(&self.password)
                .map(|(algo, session_key)| decrypt(algo, &session_key))
                .unwrap_or(false)
            {
                return Ok(None);
            }
        }
        Err(io::Error::other("senha incorreta").into())
    }
}

pub struct DecryptOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub password: Password,
    pub overwrite: bool,
    pub restrict_permissions: bool,
    pub cancel: Arc<AtomicBool>,
    pub progress_interval: Duration,
    pub on_progress: Box<dyn Fn(ProgressSnapshot) + Send>,
}

#[derive(Debug)]
pub struct DecryptOutcome {
    pub output: PathBuf,
    pub input_bytes: u64,
    pub output_bytes: u64,
}

struct CountingReader<R> {
    inner: R,
    read_bytes: Arc<AtomicU64>,
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read_bytes
            .fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

/// Confere a senha só no SKESK, sem descriptografar o conteúdo.
pub fn password_opens(path: &Path, password: &Password) -> Result<(), CryptoError> {
    let file = File::open(path).map_err(map_read_error)?;
    let mut parser = PacketParser::from_reader(file).map_err(pgp)?;
    let mut saw_skesk = false;
    while let PacketParserResult::Some(pp) = parser {
        let tag = pp.packet.tag();
        if tag == Tag::SKESK {
            saw_skesk = true;
            if let Packet::SKESK(skesk) = &pp.packet {
                if skesk.decrypt(password).is_ok() {
                    return Ok(());
                }
            }
        }
        if tag == Tag::SEIP {
            break;
        }
        parser = pp.next().map_err(pgp)?.1;
    }
    if saw_skesk {
        Err(CryptoError::OpenPgp("senha não abre a mensagem".into()))
    } else {
        Err(CryptoError::OpenPgp("mensagem OpenPGP inválida".into()))
    }
}

/// Descriptografa em streaming. O arquivo `.gpg` original não é alterado.
pub fn decrypt_streaming(options: DecryptOptions) -> Result<DecryptOutcome, CryptoError> {
    let meta = fs::metadata(&options.input).map_err(map_read_error)?;
    if !meta.is_file() {
        return Err(CryptoError::NotAFile);
    }
    if conflicts_with_input(&options.input, &options.output)? {
        return Err(CryptoError::RefusingToOverwriteInput);
    }
    if options.output.exists() && !options.overwrite {
        return Err(CryptoError::DestinationExists);
    }
    password_opens(&options.input, &options.password)?;

    let total = meta.len();
    let mut estimator = ProgressEstimator::new(total, options.progress_interval);
    (options.on_progress)(estimator.snapshot(ProgressStatus::Preparing));
    if options.cancel.load(Ordering::Relaxed) {
        return Err(CryptoError::Cancelled);
    }

    let partial_path = partial_output(&options.output);
    if partial_path.exists() {
        fs::remove_file(&partial_path).map_err(CryptoError::CreateOutput)?;
    }
    let mut guard = PartialFile {
        path: partial_path.clone(),
        armed: true,
    };
    let mut output_file = File::create(&partial_path).map_err(CryptoError::CreateOutput)?;
    let read_bytes = Arc::new(AtomicU64::new(0));
    let input = File::open(&options.input).map_err(map_read_error)?;
    let counted = CountingReader {
        inner: input,
        read_bytes: read_bytes.clone(),
    };
    let helper = PasswordHelper {
        password: options.password.clone(),
    };
    let policy = StandardPolicy::new();
    let mut decryptor = DecryptorBuilder::from_reader(counted)
        .map_err(pgp)?
        .with_policy(&policy, None, helper)
        .map_err(pgp)?;

    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut written = 0u64;
    loop {
        if options.cancel.load(Ordering::Relaxed) {
            return Err(CryptoError::Cancelled);
        }
        let n = decryptor
            .read(&mut buf)
            .map_err(|err| CryptoError::OpenPgp(err.to_string()))?;
        if n == 0 {
            break;
        }
        output_file
            .write_all(&buf[..n])
            .map_err(CryptoError::CreateOutput)?;
        written = written.saturating_add(n as u64);
        let processed = read_bytes.load(Ordering::Relaxed).min(total);
        if let Some(snapshot) = estimator.push(processed, ProgressStatus::Encrypting, false) {
            (options.on_progress)(snapshot);
        }
    }
    if options.cancel.load(Ordering::Relaxed) {
        return Err(CryptoError::Cancelled);
    }
    (options.on_progress)(estimator.snapshot(ProgressStatus::Finalizing));
    output_file.sync_all().map_err(CryptoError::CreateOutput)?;
    drop(output_file);
    drop(decryptor);
    replace_file(&partial_path, &options.output)?;
    guard.armed = false;
    if options.restrict_permissions {
        crate::filesystem::restrict_file_permissions(&options.output)?;
    }
    let output_bytes = fs::metadata(&options.output)
        .map_err(CryptoError::CreateOutput)?
        .len();
    let _ = estimator.push(total, ProgressStatus::Completed, true);
    Ok(DecryptOutcome {
        output: options.output,
        input_bytes: total,
        output_bytes,
    })
}

/// Descriptografa um arquivo OpenPGP simétrico para outro caminho.
pub fn decrypt_file(input: &Path, output: &Path, password: &Password) -> Result<(), CryptoError> {
    decrypt_streaming(DecryptOptions {
        input: input.to_path_buf(),
        output: output.to_path_buf(),
        password: password.clone(),
        overwrite: true,
        restrict_permissions: false,
        cancel: Arc::new(AtomicBool::new(false)),
        progress_interval: Duration::from_secs(1),
        on_progress: Box::new(|_| {}),
    })?;
    Ok(())
}

pub fn assert_openpgp_message(path: &Path) -> Result<(), CryptoError> {
    let mut parser = PacketParser::from_file(path).map_err(pgp)?;
    let mut saw_skesk = false;
    let mut saw_seip = false;
    while let PacketParserResult::Some(pp) = parser {
        match pp.packet.tag() {
            Tag::SKESK => saw_skesk = true,
            Tag::SEIP => saw_seip = true,
            _ => {}
        }
        parser = pp.recurse().map_err(pgp)?.1;
    }
    if saw_skesk && saw_seip {
        Ok(())
    } else {
        Err(CryptoError::OpenPgp(
            "mensagem sem SKESK e SEIPD".to_string(),
        ))
    }
}

pub fn encrypt_password(secret: &str) -> Password {
    Password::from(secret)
}

/// Criptografa um texto curto com o mesmo OpenPGP simétrico dos arquivos e devolve ASCII armor.
pub fn encrypt_text(text: &str, password: &str) -> Result<String, CryptoError> {
    let length = text.chars().count();
    if length == 0 {
        return Err(CryptoError::EmptyText);
    }
    if length > MAX_TEXT_MESSAGE_LENGTH {
        return Err(CryptoError::MessageTooLong);
    }
    if password.is_empty() {
        return Err(CryptoError::EmptyPassword);
    }
    let password = Password::from(password);
    let mut output = Vec::new();
    {
        let message = Message::new(&mut output);
        let message = Armorer::new(message)
            .kind(ArmorKind::Message)
            .build()
            .map_err(pgp)?;
        let message = Encryptor::with_passwords(message, Some(password))
            .symmetric_algo(SymmetricAlgorithm::AES256)
            .build()
            .map_err(pgp)?;
        let mut literal = LiteralWriter::new(message)
            .format(DataFormat::Binary)
            .build()
            .map_err(pgp)?;
        literal
            .write_all(text.as_bytes())
            .map_err(|err| CryptoError::OpenPgp(err.to_string()))?;
        literal.finalize().map_err(pgp)?;
    }
    String::from_utf8(output).map_err(|_| CryptoError::InvalidMessage)
}

/// Descriptografa um bloco `-----BEGIN PGP MESSAGE-----` com a mesma política dos arquivos.
pub fn decrypt_text(armored: &str, password: &str) -> Result<String, CryptoError> {
    let armored = armored.trim();
    if armored.is_empty() {
        return Err(CryptoError::EmptyText);
    }
    if password.is_empty() {
        return Err(CryptoError::EmptyPassword);
    }
    if !armored.contains("-----BEGIN PGP MESSAGE-----") || !armored.contains("-----END PGP MESSAGE-----") {
        return Err(CryptoError::InvalidMessage);
    }
    let password = Password::from(password);
    let helper = PasswordHelper {
        password: password.clone(),
    };
    let policy = StandardPolicy::new();
    let mut decryptor = match DecryptorBuilder::from_bytes(armored.as_bytes()) {
        Ok(builder) => match builder.with_policy(&policy, None, helper) {
            Ok(decryptor) => decryptor,
            Err(err) => return Err(password_or_invalid(&err.to_string())),
        },
        Err(_) => return Err(CryptoError::InvalidMessage),
    };
    let mut plain = Vec::new();
    decryptor
        .read_to_end(&mut plain)
        .map_err(|err| password_or_invalid(&err.to_string()))?;
    String::from_utf8(plain).map_err(|_| CryptoError::InvalidMessage)
}

fn password_or_invalid(detail: &str) -> CryptoError {
    let folded = detail.to_lowercase();
    if folded.contains("senha")
        || folded.contains("password")
        || folded.contains("passphrase")
        || folded.contains("decrypt")
    {
        CryptoError::WrongPassword
    } else {
        CryptoError::InvalidMessage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::process::{Command, Stdio};

    fn password() -> Password {
        Password::from("correta-e-longa")
    }

    fn hash_file(path: &Path) -> Vec<u8> {
        let bytes = fs::read(path).unwrap();
        Sha256::digest(bytes).to_vec()
    }

    fn run_encrypt(
        input: &Path,
        output: &Path,
        secret: &Password,
        overwrite: bool,
        cancel: Arc<AtomicBool>,
        on_progress: Box<dyn Fn(ProgressSnapshot) + Send>,
    ) -> Result<EncryptOutcome, CryptoError> {
        encrypt_file(EncryptOptions {
            input: input.to_path_buf(),
            output: output.to_path_buf(),
            password: secret.clone(),
            overwrite,
            cancel,
            progress_interval: Duration::from_millis(100),
            on_progress,
        })
    }

    fn gpg_decrypt(gpg_path: &Path, output: &Path, secret: &str, home: &Path) -> bool {
        fs::create_dir_all(home).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(home, fs::Permissions::from_mode(0o700));
        }
        let mut child = Command::new("gpg")
            .args([
                "--batch",
                "--yes",
                "--pinentry-mode",
                "loopback",
                "--passphrase-fd",
                "0",
                "--decrypt",
                "--output",
            ])
            .arg(output)
            .arg(gpg_path)
            .env("GNUPGHOME", home)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("gpg precisa estar instalado para o teste de interoperabilidade");
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(secret.as_bytes())
            .unwrap();
        drop(child.stdin.take());
        child.wait().unwrap().success()
    }

    #[test]
    fn encrypts_small_file_and_gpg_recovers_it() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("documento.pdf");
        let output = dir.path().join("documento.pdf.gpg");
        let recovered = dir.path().join("recuperado.pdf");
        let original = b"%PDF-1.4 conteudo pequeno de teste\n";
        fs::write(&input, original).unwrap();
        let before = hash_file(&input);

        let outcome = run_encrypt(
            &input,
            &output,
            &password(),
            false,
            Arc::new(AtomicBool::new(false)),
            Box::new(|_| {}),
        )
        .unwrap();

        assert_eq!(hash_file(&input), before);
        assert_eq!(fs::read(&input).unwrap(), original);
        assert!(output.exists());
        assert_eq!(outcome.input_bytes, original.len() as u64);
        assert_openpgp_message(&output).unwrap();

        decrypt_file(&output, &recovered, &password()).unwrap();
        assert_eq!(fs::read(&recovered).unwrap(), original);

        let gpg_out = dir.path().join("via-gpg.pdf");
        assert!(gpg_decrypt(
            &output,
            &gpg_out,
            "correta-e-longa",
            &dir.path().join("gnupg")
        ));
        assert_eq!(fs::read(&gpg_out).unwrap(), original);
    }

    #[test]
    fn encrypts_a_file_larger_than_one_chunk() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("grande.bin");
        let output = filesystem::suggested_output(&input);
        let mut file = File::create(&input).unwrap();
        let chunk = vec![0x5Au8; CHUNK_SIZE];
        for _ in 0..8 {
            file.write_all(&chunk).unwrap();
        }
        drop(file);
        let before = hash_file(&input);

        run_encrypt(
            &input,
            &output,
            &password(),
            false,
            Arc::new(AtomicBool::new(false)),
            Box::new(|_| {}),
        )
        .unwrap();

        assert_eq!(hash_file(&input), before);
        assert_openpgp_message(&output).unwrap();
        let recovered = dir.path().join("grande.recuperado");
        decrypt_file(&output, &recovered, &password()).unwrap();
        assert_eq!(hash_file(&recovered), before);

        let gpg_out = dir.path().join("grande.gpg-out");
        assert!(gpg_decrypt(
            &output,
            &gpg_out,
            "correta-e-longa",
            &dir.path().join("gnupg-large")
        ));
        assert_eq!(hash_file(&gpg_out), before);
    }

    #[test]
    fn wrong_password_fails_in_sequoia_and_gpg() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("segredo.txt");
        let output = dir.path().join("segredo.txt.gpg");
        fs::write(&input, b"dado sensivel").unwrap();
        run_encrypt(
            &input,
            &output,
            &password(),
            false,
            Arc::new(AtomicBool::new(false)),
            Box::new(|_| {}),
        )
        .unwrap();

        let bad = Password::from("errada");
        let recovered = dir.path().join("nao-deve-existir");
        assert!(decrypt_file(&output, &recovered, &bad).is_err());
        assert!(!gpg_decrypt(
            &output,
            &dir.path().join("gpg-errado"),
            "errada",
            &dir.path().join("gnupg-bad")
        ));
    }

    #[test]
    fn missing_file_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let err = run_encrypt(
            &dir.path().join("nao-existe.txt"),
            &dir.path().join("nao-existe.txt.gpg"),
            &password(),
            false,
            Arc::new(AtomicBool::new(false)),
            Box::new(|_| {}),
        )
        .unwrap_err();
        assert!(matches!(err, CryptoError::NotFound));
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_file_is_reported() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("bloqueado.txt");
        fs::write(&input, b"oculto").unwrap();
        fs::set_permissions(&input, fs::Permissions::from_mode(0o000)).unwrap();
        let err = run_encrypt(
            &input,
            &dir.path().join("bloqueado.txt.gpg"),
            &password(),
            false,
            Arc::new(AtomicBool::new(false)),
            Box::new(|_| {}),
        )
        .unwrap_err();
        fs::set_permissions(&input, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(err, CryptoError::PermissionDenied));
    }

    #[test]
    fn existing_destination_is_left_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("nota.txt");
        let output = dir.path().join("nota.txt.gpg");
        fs::write(&input, b"novo").unwrap();
        fs::write(&output, b"destino anterior").unwrap();
        let err = run_encrypt(
            &input,
            &output,
            &password(),
            false,
            Arc::new(AtomicBool::new(false)),
            Box::new(|_| {}),
        )
        .unwrap_err();
        assert!(matches!(err, CryptoError::DestinationExists));
        assert_eq!(fs::read(&output).unwrap(), b"destino anterior");
        assert_eq!(fs::read(&input).unwrap(), b"novo");
    }

    #[test]
    fn cancellation_removes_the_partial_file() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("longo.bin");
        let output = dir.path().join("longo.bin.gpg");
        let mut file = File::create(&input).unwrap();
        let chunk = vec![0x11u8; CHUNK_SIZE];
        for _ in 0..6 {
            file.write_all(&chunk).unwrap();
        }
        drop(file);
        let before = fs::read(&input).unwrap();

        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let err = encrypt_file(EncryptOptions {
            input: input.clone(),
            output: output.clone(),
            password: password(),
            overwrite: false,
            cancel,
            progress_interval: Duration::ZERO,
            on_progress: Box::new(move |snapshot| {
                if snapshot.processed_bytes > 0 {
                    flag.store(true, Ordering::Relaxed);
                }
            }),
        })
        .unwrap_err();

        assert!(matches!(err, CryptoError::Cancelled));
        assert!(!output.exists());
        assert!(!partial_output(&output).exists());
        assert_eq!(fs::read(&input).unwrap(), before);
    }

    #[test]
    fn refuses_to_replace_the_original_file() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("unico.txt");
        fs::write(&input, b"original").unwrap();
        let err = run_encrypt(
            &input,
            &input,
            &password(),
            true,
            Arc::new(AtomicBool::new(false)),
            Box::new(|_| {}),
        )
        .unwrap_err();
        assert!(matches!(err, CryptoError::RefusingToOverwriteInput));
        assert_eq!(fs::read(&input).unwrap(), b"original");
    }

    fn gpg_encrypt(input: &Path, output: &Path, secret: &str, home: &Path) {
        fs::create_dir_all(home).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(home, fs::Permissions::from_mode(0o700));
        }
        let mut child = Command::new("gpg")
            .args([
                "--batch",
                "--yes",
                "--pinentry-mode",
                "loopback",
                "--passphrase-fd",
                "0",
                "--rfc4880",
                "--force-mdc",
                "--symmetric",
                "--cipher-algo",
                "AES256",
                "--output",
            ])
            .arg(output)
            .arg(input)
            .env("GNUPGHOME", home)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("gpg precisa estar instalado");
        child.stdin.as_mut().unwrap().write_all(secret.as_bytes()).unwrap();
        drop(child.stdin.take());
        assert!(child.wait().unwrap().success(), "gpg não criptografou");
    }

    #[test]
    fn gpg_ciphertext_is_decrypted_by_the_app() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("origem.txt");
        let gpg = dir.path().join("origem.txt.gpg");
        let recovered = dir.path().join("origem.recuperado");
        fs::write(&input, "conteúdo com unicode café".as_bytes()).unwrap();
        let before = hash_file(&input);
        gpg_encrypt(&input, &gpg, "correta-e-longa", &dir.path().join("gnupg-ext"));
        decrypt_file(&gpg, &recovered, &password()).unwrap();
        assert_eq!(hash_file(&recovered), before);
        assert_eq!(hash_file(&gpg), hash_file(&gpg));
        let gpg_before = hash_file(&gpg);
        assert_eq!(hash_file(&gpg), gpg_before);
    }

    #[test]
    fn decrypt_rejects_wrong_password_corrupt_missing_and_existing() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("dado.txt");
        let gpg = dir.path().join("dado.txt.gpg");
        fs::write(&input, b"abc").unwrap();
        run_encrypt(&input, &gpg, &password(), false, Arc::new(AtomicBool::new(false)), Box::new(|_| {})).unwrap();
        let gpg_hash = hash_file(&gpg);

        assert!(decrypt_file(&gpg, &dir.path().join("ruim"), &Password::from("errada")).is_err());
        assert_eq!(hash_file(&gpg), gpg_hash);

        fs::write(dir.path().join("quebrado.gpg"), b"isto nao e openpgp").unwrap();
        assert!(decrypt_file(
            &dir.path().join("quebrado.gpg"),
            &dir.path().join("nao"),
            &password()
        )
        .is_err());

        let missing = decrypt_streaming(DecryptOptions {
            input: dir.path().join("ausente.gpg"),
            output: dir.path().join("ausente"),
            password: password(),
            overwrite: false,
            restrict_permissions: false,
            cancel: Arc::new(AtomicBool::new(false)),
            progress_interval: Duration::from_millis(100),
            on_progress: Box::new(|_| {}),
        })
        .unwrap_err();
        assert!(matches!(missing, CryptoError::NotFound));

        let occupied = dir.path().join("ocupado");
        fs::write(&occupied, b"ja existe").unwrap();
        let err = decrypt_streaming(DecryptOptions {
            input: gpg.clone(),
            output: occupied.clone(),
            password: password(),
            overwrite: false,
            restrict_permissions: false,
            cancel: Arc::new(AtomicBool::new(false)),
            progress_interval: Duration::from_millis(100),
            on_progress: Box::new(|_| {}),
        })
        .unwrap_err();
        assert!(matches!(err, CryptoError::DestinationExists));
        assert_eq!(fs::read(&occupied).unwrap(), b"ja existe");
        assert_eq!(hash_file(&gpg), gpg_hash);
    }

    #[cfg(unix)]
    #[test]
    fn decrypt_reports_unreadable_ciphertext() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("x.txt");
        let gpg = dir.path().join("x.txt.gpg");
        fs::write(&input, b"x").unwrap();
        run_encrypt(&input, &gpg, &password(), false, Arc::new(AtomicBool::new(false)), Box::new(|_| {})).unwrap();
        fs::set_permissions(&gpg, fs::Permissions::from_mode(0o000)).unwrap();
        let err = decrypt_file(&gpg, &dir.path().join("out"), &password()).unwrap_err();
        fs::set_permissions(&gpg, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(err, CryptoError::PermissionDenied));
    }

    #[test]
    fn decrypt_cancellation_removes_partial_and_keeps_gpg() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("longo.bin");
        let gpg = dir.path().join("longo.bin.gpg");
        let mut file = File::create(&input).unwrap();
        let chunk = vec![0x22u8; CHUNK_SIZE];
        for _ in 0..6 {
            file.write_all(&chunk).unwrap();
        }
        drop(file);
        run_encrypt(&input, &gpg, &password(), false, Arc::new(AtomicBool::new(false)), Box::new(|_| {})).unwrap();
        let before = hash_file(&gpg);
        let output = dir.path().join("longo.bin");
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let err = decrypt_streaming(DecryptOptions {
            input: gpg.clone(),
            output: output.clone(),
            password: password(),
            overwrite: true,
            restrict_permissions: false,
            cancel,
            progress_interval: Duration::ZERO,
            on_progress: Box::new(move |snapshot| {
                if snapshot.processed_bytes > 0 {
                    flag.store(true, Ordering::Relaxed);
                }
            }),
        })
        .unwrap_err();
        assert!(matches!(err, CryptoError::Cancelled));
        assert!(!partial_output(&output).exists());
        assert_eq!(hash_file(&gpg), before);
    }

    #[test]
    fn large_unicode_and_spaced_names_roundtrip_with_gpg() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("relatório final.tar.gz");
        let mut file = File::create(&input).unwrap();
        let chunk = vec![0x33u8; CHUNK_SIZE];
        for _ in 0..4 {
            file.write_all(&chunk).unwrap();
        }
        drop(file);
        let before = hash_file(&input);
        let gpg = filesystem::suggested_output(&input);
        run_encrypt(&input, &gpg, &password(), false, Arc::new(AtomicBool::new(false)), Box::new(|_| {})).unwrap();
        let via_gpg = dir.path().join("via-gpg.tar.gz");
        assert!(gpg_decrypt(&gpg, &via_gpg, "correta-e-longa", &dir.path().join("gnupg-a")));
        assert_eq!(hash_file(&via_gpg), before);

        let external = dir.path().join("meu arquivo.txt.gpg");
        gpg_encrypt(&input, &external, "correta-e-longa", &dir.path().join("gnupg-b"));
        let recovered = dir.path().join("meu arquivo.txt");
        decrypt_file(&external, &recovered, &password()).unwrap();
        assert_eq!(hash_file(&recovered), before);
        assert_eq!(
            filesystem::suggested_plaintext_name(&external),
            "meu arquivo.txt"
        );
        assert_eq!(
            filesystem::suggested_plaintext_name(&gpg),
            "relatório final.tar.gz"
        );
    }

    #[test]
    fn text_roundtrip_preserves_unicode_and_lines() {
        let samples = [
            "hello",
            "ação, coração, não",
            "linha 1\nlinha 2\n",
            "日本語のテスト",
            "中文测试",
            "emoji 🔐✨",
            "aspas \" ' < > & / \\",
        ];
        for sample in samples {
            let armored = encrypt_text(sample, "senha segura").unwrap();
            assert!(armored.contains("-----BEGIN PGP MESSAGE-----"));
            assert!(armored.contains("-----END PGP MESSAGE-----"));
            assert_eq!(decrypt_text(&armored, "senha segura").unwrap(), sample);
        }
    }

    #[test]
    fn text_rejects_wrong_password_empty_input_and_length() {
        let armored = encrypt_text("segredo", "certa").unwrap();
        assert!(matches!(decrypt_text(&armored, "errada"), Err(CryptoError::WrongPassword)));
        assert!(matches!(encrypt_text("", "certa"), Err(CryptoError::EmptyText)));
        assert!(matches!(encrypt_text("oi", ""), Err(CryptoError::EmptyPassword)));
        assert!(matches!(decrypt_text("", "certa"), Err(CryptoError::EmptyText)));
        assert!(matches!(decrypt_text(&armored, ""), Err(CryptoError::EmptyPassword)));
        assert!(matches!(
            decrypt_text("-----BEGIN PGP MESSAGE-----\nnope\n-----END PGP MESSAGE-----", "certa"),
            Err(CryptoError::InvalidMessage)
        ));
        let exact: String = "á".repeat(MAX_TEXT_MESSAGE_LENGTH);
        assert_eq!(exact.chars().count(), MAX_TEXT_MESSAGE_LENGTH);
        let armored = encrypt_text(&exact, "certa").unwrap();
        assert_eq!(decrypt_text(&armored, "certa").unwrap(), exact);
        let too_long: String = "á".repeat(MAX_TEXT_MESSAGE_LENGTH + 1);
        assert!(matches!(encrypt_text(&too_long, "certa"), Err(CryptoError::MessageTooLong)));
    }
}
