export type PasswordIssue = "empty" | "mismatch";

export function passwordIssue(password: string, confirmation: string): PasswordIssue | null {
  if (password.length === 0) {
    return "empty";
  }
  if (password !== confirmation) {
    return "mismatch";
  }
  return null;
}

export function reminderRepeatsPassword(password: string, reminder: string): boolean {
  const hint = reminder.trim().toLowerCase();
  const secret = password.toLowerCase();
  if (!hint || !secret) {
    return false;
  }
  return hint.includes(secret);
}
