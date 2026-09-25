use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use zeroize::Zeroize;

use crate::archive;
use crate::crypto;
use crate::errors::AppError;
use crate::filesystem::{self, extension_label, file_name, looks_like_gpg, suggested_plaintext_name};

static CANCEL_FLAGS: Mutex<Option<HashMap<String, Arc<AtomicBool>>>> = Mutex::new(None);

fn flags() -> std::sync::MutexGuard<'static, Option<HashMap<String, Arc<AtomicBool>>>> {
    CANCEL_FLAGS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn register_cancel(operation_id: &str) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let mut map = flags();
    map.get_or_insert_with(HashMap::new)
        .insert(operation_id.to_string(), flag.clone());
    flag
}

fn clear_cancel(operation_id: &str) {
    if let Some(map) = flags().as_mut() {
        map.remove(operation_id);
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub extension: String,
    pub size_bytes: u64,
    pub intent: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptRequest {
    pub operation_id: String,
    pub input_paths: Vec<String>,
    pub output_path: String,
    pub password: String,
    pub overwrite: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionInfo {
    pub paths: Vec<String>,
    pub name: String,
    pub kind: String,
    pub size_bytes: u64,
    pub file_count: u64,
    pub directory_count: u64,
    pub intent: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptResponse {
    pub output_path: String,
    pub output_name: String,
    pub input_bytes: u64,
    pub output_bytes: u64,
}

#[tauri::command]
pub fn inspect_entries(paths: Vec<String>) -> Result<SelectionInfo, AppError> {
    if paths.is_empty() {
        return Err(crypto::CryptoError::NotFound.into());
    }
    let files: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let gpg = files.iter().filter(|path| looks_like_gpg(path)).count();
    if gpg > 0 && (gpg != 1 || files.len() != 1) {
        return Err(AppError::new(
            "mixed_selection",
            "Descriptografe um arquivo por vez.",
            "mixed",
        ));
    }
    if gpg == 1 {
        let info = inspect_file(paths[0].clone())?;
        return Ok(SelectionInfo {
            name: info.name,
            kind: "file".into(),
            size_bytes: info.size_bytes,
            file_count: 1,
            directory_count: 0,
            intent: "decrypt".into(),
            paths,
        });
    }
    let (size_bytes, file_count, directory_count) = archive::selection_bytes(&files)?;
    let kind = if files.len() == 1 && files[0].is_dir() {
        "directory"
    } else if files.len() == 1 {
        "file"
    } else {
        "multiple"
    };
    let name = if files.len() == 1 {
        file_name(&files[0])
    } else {
        String::new()
    };
    Ok(SelectionInfo {
        paths,
        name,
        kind: kind.into(),
        size_bytes,
        file_count,
        directory_count,
        intent: "encrypt".into(),
    })
}

#[tauri::command]
pub fn suggested_output_for(paths: Vec<String>) -> Result<String, AppError> {
    let files: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    archive::output_for(&files)
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(AppError::from)
}

#[tauri::command]
pub fn classify_plaintext(mut request: PasswordCheckRequest) -> Result<bool, AppError> {
    let password = crypto::encrypt_password(&request.password);
    request.password.zeroize();
    archive::plaintext_is_tar(PathBuf::from(&request.path).as_path(), &password).map_err(map_decrypt)
}

#[tauri::command]
pub fn prepare_open_directory() -> Result<String, AppError> {
    filesystem::prepare_temp_dir()
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(AppError::from_decrypt)
}

#[tauri::command]
pub fn inspect_file(path: String) -> Result<FileInfo, AppError> {
    let file = PathBuf::from(&path);
    let meta = std::fs::metadata(&file).map_err(|err| {
        let crypto_error = match err.kind() {
            std::io::ErrorKind::NotFound => crypto::CryptoError::NotFound,
            std::io::ErrorKind::PermissionDenied => crypto::CryptoError::PermissionDenied,
            _ => crypto::CryptoError::Read(err),
        };
        AppError::from(crypto_error)
    })?;
    if !meta.is_file() {
        return Err(crypto::CryptoError::NotAFile.into());
    }
    Ok(FileInfo {
        path,
        name: file_name(&file),
        extension: extension_label(&file),
        size_bytes: meta.len(),
        intent: if looks_like_gpg(&file) {
            "decrypt".to_string()
        } else {
            "encrypt".to_string()
        },
    })
}

#[tauri::command]
pub fn suggested_gpg_path(input_path: String) -> String {
    archive::output_for(&[PathBuf::from(input_path)])
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[tauri::command]
pub fn path_exists(path: String) -> bool {
    PathBuf::from(path).exists()
}

#[tauri::command]
pub fn suggested_plaintext_name_for(path: String) -> String {
    suggested_plaintext_name(&PathBuf::from(path))
}

#[tauri::command]
pub fn prepare_open_target(path: String) -> Result<String, AppError> {
    filesystem::prepare_temp_output(&PathBuf::from(path))
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(AppError::from_decrypt)
}

#[tauri::command]
pub fn folder_action_label() -> &'static str {
    filesystem::folder_action_label()
}

#[tauri::command]
pub fn cancel_encrypt(operation_id: String) {
    if let Some(flag) = flags()
        .as_ref()
        .and_then(|map| map.get(&operation_id))
        .cloned()
    {
        flag.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
pub async fn encrypt_file(app: AppHandle, mut request: EncryptRequest) -> Result<EncryptResponse, AppError> {
    let operation_id = request.operation_id.clone();
    let cancel = register_cancel(&operation_id);
    let password = crypto::encrypt_password(&request.password);
    request.password.zeroize();

    let inputs: Vec<PathBuf> = request.input_paths.iter().map(PathBuf::from).collect();
    let output = PathBuf::from(&request.output_path);
    let app_progress = app.clone();
    let progress_id = operation_id.clone();
    let overwrite = request.overwrite;

    let result = tauri::async_runtime::spawn_blocking(move || {
        archive::encrypt_paths(
            &inputs,
            &output,
            &password,
            overwrite,
            cancel,
            Duration::from_millis(100),
            Box::new(move |snapshot| {
                let _ = app_progress.emit("encrypt-progress", snapshot.into_event(&progress_id));
            }),
        )
    })
    .await
    .map_err(|err| AppError::new("crypto_failed", "Não foi possível concluir a criptografia.", err))?
    .map_err(AppError::from);

    clear_cancel(&operation_id);
    let outcome = result?;
    Ok(EncryptResponse {
        output_path: outcome.output.to_string_lossy().into_owned(),
        output_name: file_name(&outcome.output),
        input_bytes: outcome.input_bytes,
        output_bytes: outcome.output_bytes,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordCheckRequest {
    pub path: String,
    pub password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecryptRequest {
    pub operation_id: String,
    pub input_path: String,
    pub output_path: String,
    pub password: String,
    pub overwrite: bool,
    pub restrict_permissions: bool,
    pub extract: bool,
}

fn map_decrypt(error: crypto::CryptoError) -> AppError {
    let app = AppError::from_decrypt(error);
    if let Some(detail) = &app.detail {
        eprintln!("orangeEncrypt: {} ({})", app.message, detail);
    }
    app
}

#[tauri::command]
pub fn verify_decrypt_password(mut request: PasswordCheckRequest) -> Result<(), AppError> {
    let password = crypto::encrypt_password(&request.password);
    request.password.zeroize();
    crypto::password_opens(PathBuf::from(&request.path).as_path(), &password).map_err(map_decrypt)
}

#[tauri::command]
pub async fn decrypt_file(app: AppHandle, mut request: DecryptRequest) -> Result<EncryptResponse, AppError> {
    let operation_id = request.operation_id.clone();
    let cancel = register_cancel(&operation_id);
    let password = crypto::encrypt_password(&request.password);
    request.password.zeroize();
    let input = PathBuf::from(&request.input_path);
    let output = PathBuf::from(&request.output_path);
    let app_progress = app.clone();
    let progress_id = operation_id.clone();
    let restrict_permissions = request.restrict_permissions;
    let overwrite = request.overwrite;
    let extract = request.extract;

    let result = tauri::async_runtime::spawn_blocking(move || {
        if extract {
            let (folder, output_bytes) = archive::decrypt_archive(
                crypto::DecryptOptions {
                    input,
                    output: output.clone(),
                    password,
                    overwrite,
                    restrict_permissions,
                    cancel,
                    progress_interval: Duration::from_millis(100),
                    on_progress: Box::new(move |snapshot| {
                        let _ = app_progress.emit("encrypt-progress", snapshot.into_event(&progress_id));
                    }),
                },
                &output,
            )?;
            return Ok(crypto::DecryptOutcome {
                output: folder,
                input_bytes: 0,
                output_bytes,
            });
        }
        crypto::decrypt_streaming(crypto::DecryptOptions {
            input,
            output,
            password,
            overwrite,
            restrict_permissions,
            cancel,
            progress_interval: Duration::from_millis(100),
            on_progress: Box::new(move |snapshot| {
                let _ = app_progress.emit("encrypt-progress", snapshot.into_event(&progress_id));
            }),
        })
    })
    .await
    .map_err(|err| {
        AppError::new(
            "decrypt_failed",
            "Não foi possível descriptografar o arquivo. Verifique a senha e tente novamente.",
            err,
        )
    })?
    .map_err(map_decrypt);

    clear_cancel(&operation_id);
    let outcome = result?;
    Ok(EncryptResponse {
        output_path: outcome.output.to_string_lossy().into_owned(),
        output_name: file_name(&outcome.output),
        input_bytes: outcome.input_bytes,
        output_bytes: outcome.output_bytes,
    })
}

#[derive(Deserialize)]
pub(crate) struct TextRequest {
    text: String,
    password: String,
}

#[tauri::command]
pub fn encrypt_text(mut request: TextRequest) -> Result<String, AppError> {
    let mut text = std::mem::take(&mut request.text);
    let mut password = std::mem::take(&mut request.password);
    let result = crypto::encrypt_text(&text, &password);
    text.zeroize();
    password.zeroize();
    result.map_err(AppError::from)
}

#[tauri::command]
pub fn decrypt_text(mut request: TextRequest) -> Result<String, AppError> {
    let mut text = std::mem::take(&mut request.text);
    let mut password = std::mem::take(&mut request.password);
    let result = crypto::decrypt_text(&text, &password);
    text.zeroize();
    password.zeroize();
    result.map_err(AppError::from)
}
