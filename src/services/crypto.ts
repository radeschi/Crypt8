import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { messages } from "../i18n/index.ts";
import type { AppError, EncryptResult, FileInfo, ProgressEvent, SelectionInfo } from "../types/progress.ts";

export async function inspectFile(path: string): Promise<FileInfo> {
  return invoke<FileInfo>("inspect_file", { path });
}

export async function inspectEntries(paths: string[]): Promise<SelectionInfo> {
  return invoke<SelectionInfo>("inspect_entries", { paths });
}

export async function suggestedOutputFor(paths: string[]): Promise<string> {
  return invoke<string>("suggested_output_for", { paths });
}

export async function classifyPlaintext(path: string, password: string): Promise<boolean> {
  return invoke<boolean>("classify_plaintext", { request: { path, password } });
}

export async function prepareOpenDirectory(): Promise<string> {
  return invoke<string>("prepare_open_directory");
}

export async function suggestedGpgPath(inputPath: string): Promise<string> {
  return invoke<string>("suggested_gpg_path", { inputPath });
}

export async function pathExists(path: string): Promise<boolean> {
  return invoke<boolean>("path_exists", { path });
}

export async function folderActionLabel(): Promise<string> {
  return invoke<string>("folder_action_label");
}

export async function suggestedPlaintextName(path: string): Promise<string> {
  return invoke<string>("suggested_plaintext_name_for", { path });
}

export async function prepareOpenTarget(path: string): Promise<string> {
  return invoke<string>("prepare_open_target", { path });
}

export async function verifyDecryptPassword(path: string, password: string): Promise<void> {
  return invoke("verify_decrypt_password", { request: { path, password } });
}

export async function decryptFile(request: {
  operationId: string;
  inputPath: string;
  outputPath: string;
  password: string;
  overwrite: boolean;
  restrictPermissions: boolean;
  extract: boolean;
}): Promise<EncryptResult> {
  return invoke<EncryptResult>("decrypt_file", { request });
}

export async function encryptFile(request: {
  operationId: string;
  inputPaths: string[];
  outputPath: string;
  password: string;
  overwrite: boolean;
}): Promise<EncryptResult> {
  return invoke<EncryptResult>("encrypt_file", { request });
}

export async function cancelEncrypt(operationId: string): Promise<void> {
  return invoke("cancel_encrypt", { operationId });
}

export function onEncryptProgress(
  handler: (event: ProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<ProgressEvent>("encrypt-progress", (event) => {
    handler(event.payload);
  });
}

export function asAppError(error: unknown): AppError {
  if (error && typeof error === "object" && "message" in error && "code" in error) {
    const candidate = error as AppError;
    return {
      code: String(candidate.code),
      message: String(candidate.message),
      detail: candidate.detail,
    };
  }
  return {
    code: "crypto_failed",
    message: messages.errors.crypto_failed,
  };
}
