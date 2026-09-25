use std::path::{Path, PathBuf};

use crate::crypto::CryptoError;

pub fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("arquivo")
        .to_string()
}

pub fn extension_label(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_string()
}

/// Nome interno do pacote literal OpenPGP. O tamanho máximo do campo é 255 bytes.
pub fn literal_filename(path: &Path) -> String {
    let name = file_name(path);
    let mut out = String::new();
    for ch in name.chars() {
        if out.len() + ch.len_utf8() > 255 {
            break;
        }
        out.push(ch);
    }
    if out.is_empty() {
        "arquivo".to_string()
    } else {
        out
    }
}

pub fn looks_like_gpg(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gpg"))
}

/// Remove só o sufixo `.gpg`, preservando extensões internas, espaços e Unicode.
pub fn suggested_plaintext_name(path: &Path) -> String {
    let name = file_name(path);
    match strip_gpg_suffix(&name) {
        Some(stripped) if !stripped.is_empty() => stripped.to_string(),
        _ if looks_like_gpg(path) => "arquivo".to_string(),
        _ => name,
    }
}

fn strip_gpg_suffix(name: &str) -> Option<&str> {
    let bytes = name.as_bytes();
    if bytes.len() >= 4
        && bytes[bytes.len() - 4] == b'.'
        && bytes[bytes.len() - 3..].eq_ignore_ascii_case(b"gpg")
    {
        Some(&name[..name.len() - 4])
    } else {
        None
    }
}

pub fn suggested_output(input: &Path) -> PathBuf {
    let mut name = file_name(input);
    name.push_str(".gpg");
    input.with_file_name(name)
}

pub fn partial_output(output: &Path) -> PathBuf {
    let mut name = file_name(output);
    name.push_str(".partial");
    output.with_file_name(name)
}

pub fn conflicts_with_input(input: &Path, output: &Path) -> Result<bool, CryptoError> {
    if input == output {
        return Ok(true);
    }
    if output.exists() {
        let input_real = input.canonicalize().map_err(CryptoError::Read)?;
        let output_real = output.canonicalize().map_err(CryptoError::Read)?;
        return Ok(input_real == output_real);
    }
    Ok(false)
}

use std::sync::Mutex;

static TEMP_OUTPUTS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Cria um caminho em um diretório temporário do sistema, fora da pasta do aplicativo.
pub fn prepare_temp_output(source: &Path) -> Result<PathBuf, CryptoError> {
    let dir = std::env::temp_dir().join(format!(
        "orange-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs_create_dir(&dir)?;
    let path = dir.join(suggested_plaintext_name(source));
    TEMP_OUTPUTS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .push(path.clone());
    Ok(path)
}

fn fs_create_dir(dir: &Path) -> Result<(), CryptoError> {
    std::fs::create_dir_all(dir).map_err(CryptoError::CreateOutput)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(CryptoError::CreateOutput)?;
    }
    Ok(())
}

pub fn restrict_file_permissions(path: &Path) -> Result<(), CryptoError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(CryptoError::CreateOutput)?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// Remove os arquivos temporários criados por "Abrir". Não é apagamento seguro.
pub fn cleanup_temporary_files() {
    let paths = std::mem::take(&mut *TEMP_OUTPUTS.lock().unwrap_or_else(|p| p.into_inner()));
    for path in paths {
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(&path);
        } else {
            let _ = std::fs::remove_file(&path);
            if let Some(parent) = path.parent() {
                let _ = std::fs::remove_dir(parent);
            }
        }
    }
}

/// Rótulo do botão que revela o arquivo, isolado por plataforma.
pub fn folder_action_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "finder"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "folder"
    }
}

pub fn prepare_temp_dir() -> Result<PathBuf, CryptoError> {
    let dir = std::env::temp_dir().join(format!(
        "crypt8-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs_create_dir(&dir)?;
    TEMP_OUTPUTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(dir.clone());
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_gpg_without_touching_the_original_name() {
        let output = suggested_output(Path::new("/tmp/documento.pdf"));
        assert_eq!(output.file_name().unwrap(), "documento.pdf.gpg");
    }

    #[test]
    fn suggests_plaintext_names_without_blindly_stripping() {
        assert_eq!(
            suggested_plaintext_name(Path::new("/tmp/documento.pdf.gpg")),
            "documento.pdf"
        );
        assert_eq!(
            suggested_plaintext_name(Path::new("backup.tar.gz.gpg")),
            "backup.tar.gz"
        );
        assert_eq!(suggested_plaintext_name(Path::new("arquivo.gpg")), "arquivo");
        assert_eq!(
            suggested_plaintext_name(Path::new("relatório final.pdf.GPG")),
            "relatório final.pdf"
        );
        assert_eq!(
            suggested_plaintext_name(Path::new("meu arquivo.txt.gpg")),
            "meu arquivo.txt"
        );
        assert!(!looks_like_gpg(Path::new("notas.txt")));
    }

    #[test]
    fn truncates_literal_names_to_the_openpgp_limit() {
        let long = "a".repeat(400);
        let path = PathBuf::from(&long);
        assert!(literal_filename(&path).len() <= 255);
    }
}
