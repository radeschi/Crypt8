import assert from "node:assert/strict";
import test from "node:test";
import { messagesFor } from "../i18n/index.ts";
import { formatBytes, formatRemaining } from "./format.ts";

test("formata tamanho em português", () => {
  assert.equal(formatBytes(0), "0 B");
  assert.equal(formatBytes(1536), "1,5 KB");
});

test("não estima cedo demais e não promete o último segundo", () => {
  const copy = messagesFor("pt-BR");
  assert.equal(formatRemaining(null, "encrypting", copy), "Calculando...");
  assert.equal(formatRemaining(0.4, "encrypting", copy), "Finalizando...");
  assert.equal(formatRemaining(12, "encrypting", copy), "~12 segundos restantes");
  assert.equal(formatRemaining(125, "encrypting", copy), "~2 minutos restantes");
  assert.equal(formatRemaining(12, "finalizing", copy), "Finalizando...");
});
