import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { updateFailure, type UpdateFailure } from "./updateErrors.ts";

export type { UpdateFailure };

export async function lookForUpdate(): Promise<Update | null> {
  return check();
}

export async function downloadAndInstall(
  update: Update,
  onPhase: (phase: "downloading" | "installing") => void,
): Promise<void> {
  onPhase("downloading");
  await update.download();
  onPhase("installing");
  await update.install();
}

export async function restartApp(): Promise<void> {
  await relaunch();
}

export function failureKind(error: unknown, phase: "check" | "install"): UpdateFailure {
  return updateFailure(error, phase);
}
