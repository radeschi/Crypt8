import assert from "node:assert/strict";
import test from "node:test";
import { passwordIssue, reminderRepeatsPassword } from "./password.ts";

test("exige uma senha", () => {
  assert.equal(passwordIssue("", ""), "empty");
});

test("a confirmação precisa ser idêntica", () => {
  assert.equal(passwordIssue("segredo", "segredo"), null);
  assert.equal(passwordIssue("segredo", "outra"), "mismatch");
});

test("o lembrete não deve repetir a senha", () => {
  assert.equal(reminderRepeatsPassword("laranja", "a fruta da casa"), false);
  assert.equal(reminderRepeatsPassword("laranja", "a senha é laranja"), true);
  assert.equal(reminderRepeatsPassword("laranja", "   "), false);
});
