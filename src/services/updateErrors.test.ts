import assert from "node:assert/strict";
import test from "node:test";
import { updateFailure } from "./updateErrors.ts";

test("rejeita assinatura inválida sem tratar como falha genérica", () => {
  assert.equal(updateFailure(new Error("invalid signature"), "install"), "signature");
  assert.equal(updateFailure(new Error("minisign verification failed"), "check"), "signature");
});

test("preserva falha de rede na verificação e na instalação", () => {
  assert.equal(updateFailure(new Error("network unreachable"), "check"), "check");
  assert.equal(updateFailure(new Error("download failed"), "install"), "install");
});
