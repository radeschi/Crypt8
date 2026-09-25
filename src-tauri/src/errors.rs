use serde::Serialize;

use crate::crypto::CryptoError;

/// Erro mostrado ao usuário. Detalhe técnico só existe em build de desenvolvimento.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: &str, message: impl Into<String>, detail: impl ToString) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            detail: if cfg!(debug_assertions) {
                Some(detail.to_string())
            } else {
                None
            },
        }
    }

    pub fn from_decrypt(error: CryptoError) -> Self {
        let (code, message) = match &error {
            CryptoError::OpenPgp(_) => (
                "decrypt_failed",
                "Não foi possível descriptografar o arquivo. Verifique a senha e tente novamente.",
            ),
            CryptoError::CreateOutput(_) => (
                "write_failed",
                "Não foi possível criar o arquivo descriptografado.",
            ),
            _ => return Self::from_crypto(error),
        };
        Self::new(code, message, error)
    }

    pub fn from_crypto(error: CryptoError) -> Self {
        let (code, message) = match &error {
            CryptoError::NotFound | CryptoError::Read(_) => {
                ("read_failed", "Não foi possível ler o arquivo.")
            }
            CryptoError::PermissionDenied => {
                ("permission_denied", "Não foi possível ler o arquivo.")
            }
            CryptoError::NotAFile => ("not_a_file", "Selecione um arquivo, não uma pasta."),
            CryptoError::DestinationExists => (
                "destination_exists",
                "Já existe um arquivo com esse nome.",
            ),
            CryptoError::RefusingToOverwriteInput => (
                "refusing_overwrite",
                "O arquivo original não pode ser substituído.",
            ),
            CryptoError::CreateOutput(_) => (
                "write_failed",
                "Não foi possível criar o arquivo criptografado.",
            ),
            CryptoError::Cancelled => ("cancelled", "A operação foi cancelada."),
            CryptoError::OpenPgp(_) => (
                "crypto_failed",
                "Não foi possível concluir a criptografia.",
            ),
            CryptoError::UnsafePath => (
                "unsafe_path",
                "O pacote contém um caminho que não pode ser extraído.",
            ),
            CryptoError::Symlink => (
                "symlink",
                "Links simbólicos não são aceitos.",
            ),
            CryptoError::SpecialFile => (
                "special_file",
                "Arquivos especiais não são aceitos.",
            ),
            CryptoError::EmptyText => ("empty_text", "Nenhum conteúdo informado."),
            CryptoError::EmptyPassword => ("empty_password", "Informe uma senha."),
            CryptoError::MessageTooLong => ("text_too_long", "A mensagem é longa demais."),
            CryptoError::InvalidMessage => ("invalid_message", "Mensagem OpenPGP inválida."),
            CryptoError::WrongPassword => ("wrong_password", "Senha incorreta."),
        };
        Self::new(code, message, error)
    }
}

impl From<CryptoError> for AppError {
    fn from(error: CryptoError) -> Self {
        let app = Self::from_crypto(error);
        if let Some(detail) = &app.detail {
            eprintln!("orangeEncrypt: {} ({})", app.message, detail);
        }
        app
    }
}
