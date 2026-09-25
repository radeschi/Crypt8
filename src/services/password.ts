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
