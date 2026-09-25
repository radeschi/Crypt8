import assert from "node:assert/strict";
import test from "node:test";
import { passwordIssue } from "./password.ts";

test("exige uma senha", () => {
  assert.equal(passwordIssue("", ""), "empty");
});

test("a confirmação precisa ser idêntica", () => {
  assert.equal(passwordIssue("segredo", "segredo"), null);
  assert.equal(passwordIssue("segredo", "outra"), "mismatch");
});
