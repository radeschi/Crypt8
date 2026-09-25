import { invoke } from "@tauri-apps/api/core";

export const MAX_TEXT_MESSAGE_LENGTH = 2000;

export async function encryptText(text: string, password: string): Promise<string> {
  return invoke<string>("encrypt_text", { request: { text, password } });
}

export async function decryptText(text: string, password: string): Promise<string> {
  return invoke<string>("decrypt_text", { request: { text, password } });
}

export function characterCount(value: string): number {
  return Array.from(value).length;
}

export function clampToMessageLength(value: string): string {
  const chars = Array.from(value);
  if (chars.length <= MAX_TEXT_MESSAGE_LENGTH) return value;
  return chars.slice(0, MAX_TEXT_MESSAGE_LENGTH).join("");
}

export function looksLikeOpenPgpMessage(value: string): boolean {
  return value.includes("-----BEGIN PGP MESSAGE-----");
}
