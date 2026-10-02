// Developed by: Maicon Radeschi
// Email: radeschi@me.com
// May the Force be with you!
// 2026

export type UpdateFailure = "signature" | "check" | "install";

export function updateFailure(error: unknown, phase: "check" | "install"): UpdateFailure {
  const text = errorText(error);
  if (
    text.includes("signature") ||
    text.includes("minisign") ||
    text.includes("pubkey") ||
    text.includes("public key") ||
    text.includes("assinatura")
  ) {
    return "signature";
  }
  return phase;
}

function errorText(error: unknown): string {
  if (error instanceof Error) {
    return error.message.toLowerCase();
  }
  return String(error).toLowerCase();
}
