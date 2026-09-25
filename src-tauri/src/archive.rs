use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sequoia_openpgp::crypto::Password;
use tar::{Builder, EntryType, Header};

use crate::crypto::{self, CryptoError, DecryptOptions, EncryptOutcome};
use crate::filesystem::{self, file_name};
use crate::progress::ProgressSnapshot;

const BLOCK: u64 = 512;

#[derive(Debug)]
struct Planned {
    source: PathBuf,
    archive_path: String,
    is_dir: bool,
    size: u64,
}

pub fn output_for(paths: &[PathBuf]) -> Result<PathBuf, CryptoError> {
    if paths.len() == 1 {
        let Ok(meta) = fs::symlink_metadata(&paths[0]) else {
            return Ok(filesystem::suggested_output(&paths[0]));
        };
        if meta.file_type().is_symlink() {
            return Err(CryptoError::Symlink);
        }
        if meta.is_file() {
            return Ok(filesystem::suggested_output(&paths[0]));
        }
        if meta.is_dir() {
            let name = format!("{}.tar.gpg", file_name(&paths[0]));
            return Ok(paths[0].parent().unwrap_or(Path::new(".")).join(name));
        }
        return Err(CryptoError::SpecialFile);
    }
    let parent = paths[0].parent().unwrap_or(Path::new("."));
    Ok(parent.join("Archive.tar.gpg"))
}

pub fn selection_bytes(paths: &[PathBuf]) -> Result<(u64, u64, u64), CryptoError> {
    let planned = plan(paths)?;
    let bytes = planned.iter().filter(|item| !item.is_dir).map(|item| item.size).sum();
    let files = planned.iter().filter(|item| !item.is_dir).count() as u64;
    let dirs = paths
        .iter()
        .filter(|path| fs::symlink_metadata(path).ok().is_some_and(|meta| meta.is_dir()))
        .count() as u64;
    Ok((bytes, files, dirs))
}

pub fn encrypt_paths(
    paths: &[PathBuf],
    output: &Path,
    password: &Password,
    overwrite: bool,
    cancel: Arc<AtomicBool>,
    progress_interval: Duration,
    on_progress: Box<dyn Fn(ProgressSnapshot) + Send>,
) -> Result<EncryptOutcome, CryptoError> {
    if paths.len() == 1 && fs::symlink_metadata(&paths[0]).map_err(map_io)?.is_file() {
        return crypto::encrypt_file(crypto::EncryptOptions {
            input: paths[0].clone(),
            output: output.to_path_buf(),
            password: password.clone(),
            overwrite,
            cancel,
            progress_interval,
            on_progress,
        });
    }

    for input in paths {
        if filesystem::conflicts_with_input(input, output)? || path_is_inside(output, input) {
            return Err(CryptoError::RefusingToOverwriteInput);
        }
    }
    let planned = plan(paths)?;
    let total = tar_size(&planned);
    let literal = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Archive.tar.gpg")
        .trim_end_matches(".gpg")
        .to_string();
    let cancel_write = cancel.clone();
    crypto::encrypt_generated(
        output,
        &literal,
        total,
        password,
        overwrite,
        &cancel,
        progress_interval,
        &on_progress,
        |writer| write_tar(writer, &planned, &cancel_write),
    )
}

pub fn plaintext_is_tar(path: &Path, password: &Password) -> Result<bool, CryptoError> {
    let prefix = crypto::read_plaintext_prefix(path, password, BLOCK as usize)?;
    Ok(is_tar_block(&prefix))
}

pub fn decrypt_archive(
    options: DecryptOptions,
    destination_dir: &Path,
) -> Result<(PathBuf, u64), CryptoError> {
    if !destination_dir.is_dir() {
        return Err(CryptoError::CreateOutput(io::Error::new(
            io::ErrorKind::NotFound,
            "pasta de destino inexistente",
        )));
    }
    let stem = archive_stem(&options.input);
    let temp = std::env::temp_dir().join(format!(
        "orangeencrypt-{}-{}.tar",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let guard = TempPath(temp.clone());
    let outcome = crypto::decrypt_streaming(DecryptOptions {
        output: temp.clone(),
        ..options
    })?;
    if !is_tar_file(&temp)? {
        return Err(CryptoError::OpenPgp("conteúdo não é um tar".into()));
    }
    let extracted = extract_tar(&temp, destination_dir, &stem, &outcome.output)?;
    drop(guard);
    Ok((extracted, outcome.output_bytes))
}

struct TempPath(PathBuf);

impl Drop for TempPath {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn write_tar(writer: &mut dyn Write, planned: &[Planned], cancel: &AtomicBool) -> Result<(), CryptoError> {
    let mut builder = Builder::new(writer);
    for item in planned {
        if cancel.load(Ordering::Relaxed) {
            return Err(CryptoError::Cancelled);
        }
        let mut header = Header::new_gnu();
        header.set_mode(if item.is_dir { 0o755 } else { 0o644 });
        header.set_size(if item.is_dir { 0 } else { item.size });
        header.set_entry_type(if item.is_dir {
            EntryType::Directory
        } else {
            EntryType::Regular
        });
        header.set_cksum();
        if item.is_dir {
            builder
                .append_data(&mut header, &item.archive_path, io::empty())
                .map_err(map_write)?;
        } else {
            let file = File::open(&item.source).map_err(map_io)?;
            builder
                .append_data(&mut header, &item.archive_path, file)
                .map_err(map_write)?;
        }
    }
    builder.finish().map_err(map_write)?;
    Ok(())
}

fn plan(paths: &[PathBuf]) -> Result<Vec<Planned>, CryptoError> {
    let mut planned = Vec::new();
    for path in paths {
        let meta = fs::symlink_metadata(path).map_err(map_io)?;
        if meta.file_type().is_symlink() {
            return Err(CryptoError::Symlink);
        }
        if meta.is_file() {
            let name = utf8_name(path)?;
            planned.push(Planned {
                source: path.clone(),
                archive_path: name,
                is_dir: false,
                size: meta.len(),
            });
        } else if meta.is_dir() {
            let root = utf8_name(path)?;
            planned.push(Planned {
                source: path.clone(),
                archive_path: root.clone(),
                is_dir: true,
                size: 0,
            });
            walk_dir(path, &root, &mut planned)?;
        } else {
            return Err(CryptoError::SpecialFile);
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    for item in &planned {
        if !seen.insert(item.archive_path.clone()) {
            return Err(CryptoError::UnsafePath);
        }
    }
    Ok(planned)
}

fn walk_dir(dir: &Path, prefix: &str, planned: &mut Vec<Planned>) -> Result<(), CryptoError> {
    let mut children = fs::read_dir(dir).map_err(map_io)?.collect::<Result<Vec<_>, _>>().map_err(map_io)?;
    children.sort_by_key(|entry| entry.file_name());
    for entry in children {
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(map_io)?;
        if meta.file_type().is_symlink() {
            return Err(CryptoError::Symlink);
        }
        let name = entry.file_name();
        let name = name.to_str().ok_or(CryptoError::UnsafePath)?;
        let archive_path = format!("{prefix}/{name}");
        if meta.is_dir() {
            planned.push(Planned {
                source: path.clone(),
                archive_path: archive_path.clone(),
                is_dir: true,
                size: 0,
            });
            walk_dir(&path, &archive_path, planned)?;
        } else if meta.is_file() {
            planned.push(Planned {
                source: path,
                archive_path,
                is_dir: false,
                size: meta.len(),
            });
        } else {
            return Err(CryptoError::SpecialFile);
        }
    }
    Ok(())
}

fn tar_size(planned: &[Planned]) -> u64 {
    let body: u64 = planned
        .iter()
        .map(|item| BLOCK + if item.is_dir { 0 } else { item.size.div_ceil(BLOCK) * BLOCK })
        .sum();
    body + BLOCK * 2
}

pub fn is_tar_block(block: &[u8]) -> bool {
    block.len() >= 262 && &block[257..262] == b"ustar" && tar_checksum_ok(block)
}

fn is_tar_file(path: &Path) -> Result<bool, CryptoError> {
    let mut file = File::open(path).map_err(map_io)?;
    let mut block = [0u8; 512];
    let n = file.read(&mut block).map_err(map_io)?;
    Ok(is_tar_block(&block[..n]))
}

fn tar_checksum_ok(block: &[u8]) -> bool {
    if block.len() < 512 {
        return false;
    }
    let stored = std::str::from_utf8(&block[148..156])
        .ok()
        .and_then(|text| u32::from_str_radix(text.trim().trim_end_matches('\0'), 8).ok());
    let Some(stored) = stored else {
        return false;
    };
    let mut sum = 0u32;
    for (index, byte) in block.iter().enumerate().take(512) {
        sum += if (148..156).contains(&index) { b' ' as u32 } else { *byte as u32 };
    }
    sum == stored
}

pub fn extract_tar(archive_path: &Path, destination_dir: &Path, stem: &str, _marker: &Path) -> Result<PathBuf, CryptoError> {
    let file = File::open(archive_path).map_err(map_io)?;
    let mut archive = tar::Archive::new(file);
    let mut names = Vec::new();
    for entry in archive.entries().map_err(map_io)? {
        let entry = entry.map_err(map_io)?;
        let path = entry.path().map_err(map_io)?;
        names.push(path.to_string_lossy().replace('\\', "/"));
    }
    let shared = shared_root(&names);
    let root = if shared.is_some() {
        destination_dir.to_path_buf()
    } else {
        destination_dir.join(stem)
    };
    if root.exists() && shared.is_none() {
        return Err(CryptoError::DestinationExists);
    }
    fs::create_dir_all(&root).map_err(CryptoError::CreateOutput)?;
    let file = File::open(archive_path).map_err(map_io)?;
    let mut archive = tar::Archive::new(file);
    for entry in archive.entries().map_err(map_io)? {
        let mut entry = entry.map_err(map_io)?;
        match entry.header().entry_type() {
            EntryType::Regular | EntryType::Continuous | EntryType::Directory => {}
            EntryType::Symlink | EntryType::Link => return Err(CryptoError::Symlink),
            _ => return Err(CryptoError::SpecialFile),
        }
        let relative = safe_relative(entry.path().map_err(map_io)?.as_ref())?;
        let full = root.join(&relative);
        if !full.starts_with(&root) {
            return Err(CryptoError::UnsafePath);
        }
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&full).map_err(CryptoError::CreateOutput)?;
        } else {
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).map_err(CryptoError::CreateOutput)?;
            }
            let mut output = File::create(&full).map_err(CryptoError::CreateOutput)?;
            io::copy(&mut entry, &mut output).map_err(CryptoError::CreateOutput)?;
        }
    }
    Ok(if let Some(name) = shared {
        destination_dir.join(name)
    } else {
        root
    })
}

fn shared_root(names: &[String]) -> Option<String> {
    let first = names.first()?.split('/').next()?.to_string();
    if first.is_empty() {
        return None;
    }
    let prefixed = format!("{first}/");
    if names.iter().all(|name| name == &first || name.starts_with(&prefixed)) {
        Some(first)
    } else {
        None
    }
}

fn safe_relative(path: &Path) -> Result<PathBuf, CryptoError> {
    if path.as_os_str().is_empty() {
        return Err(CryptoError::UnsafePath);
    }
    let text = path.to_string_lossy();
    if text.starts_with('/') || text.starts_with('\\') || text.contains(':') {
        return Err(CryptoError::UnsafePath);
    }
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                if part == ".." {
                    return Err(CryptoError::UnsafePath);
                }
                clean.push(part);
            }
            Component::CurDir => {}
            _ => return Err(CryptoError::UnsafePath),
        }
    }
    if clean.as_os_str().is_empty() {
        return Err(CryptoError::UnsafePath);
    }
    Ok(clean)
}

fn archive_stem(path: &Path) -> String {
    let name = file_name(path);
    let without_gpg = name.strip_suffix(".gpg").unwrap_or(&name);
    let without_tar = without_gpg.strip_suffix(".tar").unwrap_or(without_gpg);
    if without_tar.is_empty() {
        "Archive".to_string()
    } else {
        without_tar.to_string()
    }
}

fn utf8_name(path: &Path) -> Result<String, CryptoError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && !name.contains('/') && !name.contains('\\') && *name != "." && *name != "..")
        .map(|name| name.to_string())
        .ok_or(CryptoError::UnsafePath)
}

fn path_is_inside(path: &Path, dir: &Path) -> bool {
    path.starts_with(dir)
}

fn map_io(err: io::Error) -> CryptoError {
    match err.kind() {
        io::ErrorKind::NotFound => CryptoError::NotFound,
        io::ErrorKind::PermissionDenied => CryptoError::PermissionDenied,
        _ => CryptoError::Read(err),
    }
}

fn map_write(err: io::Error) -> CryptoError {
    if err.kind() == io::ErrorKind::Interrupted {
        CryptoError::Cancelled
    } else {
        CryptoError::CreateOutput(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::process::{Command, Stdio};

    fn secret() -> Password {
        Password::from("correta-e-longa")
    }

    fn hash_tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut files = Vec::new();
        walk_hash(dir, dir, &mut files);
        files.sort();
        files
    }

    fn walk_hash(root: &Path, dir: &Path, files: &mut Vec<(String, Vec<u8>)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                walk_hash(root, &path, files);
            } else {
                let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                let digest = Sha256::digest(fs::read(&path).unwrap()).to_vec();
                files.push((rel, digest));
            }
        }
    }

    fn sample_tree(root: &Path) {
        fs::create_dir_all(root.join("projeto/src")).unwrap();
        fs::write(root.join("projeto/README.md"), "olá arquivo").unwrap();
        fs::write(root.join("projeto/src/main.rs"), "fn main() {}\n").unwrap();
        fs::write(root.join("projeto/nome com espaço.txt"), "espaço").unwrap();
    }

    #[test]
    fn single_file_does_not_use_tar_name() {
        let output = output_for(&[PathBuf::from("/tmp/documento.pdf")]).unwrap();
        assert_eq!(output.file_name().unwrap(), "documento.pdf.gpg");
    }

    #[test]
    fn directory_and_many_inputs_use_tar_gpg() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Documentos");
        fs::create_dir(&folder).unwrap();
        let output = output_for(std::slice::from_ref(&folder)).unwrap();
        assert_eq!(output.file_name().unwrap(), "Documentos.tar.gpg");
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        let output = output_for(&[a, b]).unwrap();
        assert_eq!(output.file_name().unwrap(), "Archive.tar.gpg");
    }

    #[test]
    fn roundtrip_directory_keeps_bytes_and_names() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("origem");
        fs::create_dir(&source).unwrap();
        sample_tree(&source);
        let input = source.join("projeto");
        let before = hash_tree(&input);
        let output = output_for(std::slice::from_ref(&input)).unwrap();
        encrypt_paths(
            &[input.clone()],
            &output,
            &secret(),
            false,
            Arc::new(AtomicBool::new(false)),
            Duration::from_millis(50),
            Box::new(|_| {}),
        )
        .unwrap();
        assert!(plaintext_is_tar(&output, &secret()).unwrap());
        let restored = dir.path().join("restaurado");
        fs::create_dir(&restored).unwrap();
        let (folder, _) = decrypt_archive(
            DecryptOptions {
                input: output,
                output: dir.path().join("unused.tar"),
                password: secret(),
                overwrite: true,
                restrict_permissions: false,
                cancel: Arc::new(AtomicBool::new(false)),
                progress_interval: Duration::from_secs(1),
                on_progress: Box::new(|_| {}),
            },
            &restored,
        )
        .unwrap();
        assert_eq!(hash_tree(&folder), before);
        assert!(folder.join("src/main.rs").is_file());
        assert!(folder.join("nome com espaço.txt").is_file());
    }

    #[test]
    fn several_files_and_folders_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("contrato.pdf");
        let photos = dir.path().join("fotos");
        fs::write(&file, b"%PDF").unwrap();
        fs::create_dir(&photos).unwrap();
        fs::write(photos.join("foto.jpg"), b"jpeg").unwrap();
        let output = dir.path().join("Archive.tar.gpg");
        encrypt_paths(
            &[file.clone(), photos.clone()],
            &output,
            &secret(),
            false,
            Arc::new(AtomicBool::new(false)),
            Duration::from_millis(50),
            Box::new(|_| {}),
        )
        .unwrap();
        let restored = dir.path().join("out");
        fs::create_dir(&restored).unwrap();
        decrypt_archive(
            DecryptOptions {
                input: output,
                output: dir.path().join("tmp.tar"),
                password: secret(),
                overwrite: true,
                restrict_permissions: false,
                cancel: Arc::new(AtomicBool::new(false)),
                progress_interval: Duration::from_secs(1),
                on_progress: Box::new(|_| {}),
            },
            &restored,
        )
        .unwrap();
        assert_eq!(fs::read(restored.join("Archive/contrato.pdf")).unwrap(), b"%PDF");
        assert_eq!(fs::read(restored.join("Archive/fotos/foto.jpg")).unwrap(), b"jpeg");
    }

    #[test]
    fn empty_directory_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("vazia");
        fs::create_dir(&folder).unwrap();
        let output = output_for(std::slice::from_ref(&folder)).unwrap();
        encrypt_paths(
            &[folder],
            &output,
            &secret(),
            false,
            Arc::new(AtomicBool::new(false)),
            Duration::from_millis(50),
            Box::new(|_| {}),
        )
        .unwrap();
        let restored = dir.path().join("out");
        fs::create_dir(&restored).unwrap();
        let (path, _) = decrypt_archive(
            DecryptOptions {
                input: output,
                output: dir.path().join("tmp.tar"),
                password: secret(),
                overwrite: true,
                restrict_permissions: false,
                cancel: Arc::new(AtomicBool::new(false)),
                progress_interval: Duration::from_secs(1),
                on_progress: Box::new(|_| {}),
            },
            &restored,
        )
        .unwrap();
        assert!(path.is_dir());
    }

    #[test]
    fn rejects_symlink_and_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("real.txt");
        fs::write(&file, "x").unwrap();
        let link = dir.path().join("atalho");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&file, &link).unwrap();
        #[cfg(unix)]
        {
            let err = plan(std::slice::from_ref(&link)).unwrap_err();
            assert!(matches!(err, CryptoError::Symlink));
        }
        let raw = dir.path().join("evil.tar");
        write_header(&raw, b"../fora.txt");
        let err = extract_tar(&raw, dir.path(), "evil", &raw).unwrap_err();
        assert!(matches!(err, CryptoError::UnsafePath));
        assert!(!dir.path().join("fora.txt").exists());
        let abs = dir.path().join("abs.tar");
        write_header(&abs, b"/tmp/fora.txt");
        assert!(matches!(
            extract_tar(&abs, dir.path(), "abs", &abs).unwrap_err(),
            CryptoError::UnsafePath
        ));
    }

    fn write_header(path: &Path, name: &[u8]) {
        let mut block = vec![0u8; 512];
        block[..name.len()].copy_from_slice(name);
        block[156] = b'0';
        let size = b"00000000000";
        block[124..135].copy_from_slice(size);
        block[257..262].copy_from_slice(b"ustar");
        for byte in &mut block[148..156] {
            *byte = b' ';
        }
        let sum: u32 = block.iter().map(|byte| *byte as u32).sum();
        let text = format!("{sum:06o}\0 ");
        block[148..156].copy_from_slice(text.as_bytes());
        let mut file = File::create(path).unwrap();
        file.write_all(&block).unwrap();
        file.write_all(&[0u8; 1024]).unwrap();
    }

    #[test]
    fn cancel_removes_partial_and_keeps_sources() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("grande.bin");
        fs::write(&file, vec![7u8; 256 * 1024]).unwrap();
        let other = dir.path().join("outro.bin");
        fs::write(&other, vec![1u8; 64 * 1024]).unwrap();
        let output = dir.path().join("Archive.tar.gpg");
        let err = encrypt_paths(
            &[file.clone(), other.clone()],
            &output,
            &secret(),
            false,
            Arc::new(AtomicBool::new(true)),
            Duration::from_millis(10),
            Box::new(|_| {}),
        )
        .unwrap_err();
        assert!(matches!(err, CryptoError::Cancelled));
        assert!(!output.exists());
        assert!(!dir.path().join("Archive.tar.gpg.partial").exists());
        assert_eq!(fs::metadata(&file).unwrap().len(), 256 * 1024);
    }

    #[test]
    fn gpg_can_recover_our_tar_and_we_recover_gpg_tar() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Notas");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("a.txt"), "aaa").unwrap();
        let gpg_path = output_for(std::slice::from_ref(&folder)).unwrap();
        encrypt_paths(
            &[folder.clone()],
            &gpg_path,
            &secret(),
            false,
            Arc::new(AtomicBool::new(false)),
            Duration::from_millis(50),
            Box::new(|_| {}),
        )
        .unwrap();
        let tar_path = dir.path().join("via-gpg.tar");
        assert!(gpg_decrypt(&gpg_path, &tar_path, &dir.path().join("gnupg1")));
        let restored = dir.path().join("gpg-out");
        fs::create_dir(&restored).unwrap();
        let folder_out = extract_tar(&tar_path, &restored, "Notas", &tar_path).unwrap();
        assert_eq!(fs::read(folder_out.join("a.txt")).unwrap(), b"aaa");

        let plain_tar = dir.path().join("externo.tar");
        let status = Command::new("tar")
            .args(["-cf"])
            .arg(&plain_tar)
            .arg("-C")
            .arg(dir.path())
            .arg("Notas")
            .status()
            .unwrap();
        assert!(status.success());
        let external = dir.path().join("externo.tar.gpg");
        gpg_encrypt(&plain_tar, &external, &dir.path().join("gnupg2"));
        assert!(plaintext_is_tar(&external, &secret()).unwrap());
        let back = dir.path().join("de-gpg");
        fs::create_dir(&back).unwrap();
        let (path, _) = decrypt_archive(
            DecryptOptions {
                input: external,
                output: dir.path().join("externo-plain.tar"),
                password: secret(),
                overwrite: true,
                restrict_permissions: false,
                cancel: Arc::new(AtomicBool::new(false)),
                progress_interval: Duration::from_secs(1),
                on_progress: Box::new(|_| {}),
            },
            &back,
        )
        .unwrap();
        assert_eq!(fs::read(path.join("Notas/a.txt")).unwrap(), b"aaa");
    }

    fn gpg_decrypt(gpg_path: &Path, output: &Path, home: &Path) -> bool {
        fs::create_dir_all(home).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(home, fs::Permissions::from_mode(0o700));
        }
        let mut child = Command::new("gpg")
            .args(["--batch", "--yes", "--pinentry-mode", "loopback", "--passphrase-fd", "0", "--decrypt", "--output"])
            .arg(output)
            .arg(gpg_path)
            .env("GNUPGHOME", home)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.as_mut().unwrap().write_all(b"correta-e-longa").unwrap();
        drop(child.stdin.take());
        let output_err = child.wait_with_output().unwrap();
        if !output_err.status.success() {
            panic!("{}", String::from_utf8_lossy(&output_err.stderr));
        }
        true
    }

    fn gpg_encrypt(input: &Path, output: &Path, home: &Path) {
        fs::create_dir_all(home).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(home, fs::Permissions::from_mode(0o700));
        }
        let mut child = Command::new("gpg")
            .args([
                "--batch", "--yes", "--pinentry-mode", "loopback", "--passphrase-fd", "0",
                "--rfc4880", "--force-mdc", "--symmetric", "--cipher-algo", "AES256", "--output",
            ])
            .arg(output)
            .arg(input)
            .env("GNUPGHOME", home)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child.stdin.as_mut().unwrap().write_all(b"correta-e-longa").unwrap();
        drop(child.stdin.take());
        assert!(child.wait().unwrap().success());
    }
}
