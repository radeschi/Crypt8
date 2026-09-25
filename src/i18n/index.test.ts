import assert from "node:assert/strict";
import test from "node:test";
import { messagesFor, resolveLocale } from "./index.ts";

test("escolhe o idioma pelo locale e cai no inglês", () => {
  assert.equal(resolveLocale("en-US"), "en");
  assert.equal(resolveLocale("es-ES"), "es");
  assert.equal(resolveLocale("de-DE"), "de");
  assert.equal(resolveLocale("fr-FR"), "fr");
  assert.equal(resolveLocale("ja-JP"), "ja");
  assert.equal(resolveLocale("zh-CN"), "zh-CN");
  assert.equal(resolveLocale("zh-Hant"), "zh-CN");
  assert.equal(resolveLocale("pt-BR"), "pt-BR");
  assert.equal(resolveLocale("pt-PT"), "pt-BR");
  assert.equal(resolveLocale("it-IT"), "en");
  assert.equal(resolveLocale("ru-RU"), "en");
});

test("pluraliza arquivo e tempo", () => {
  assert.equal(messagesFor("pt-BR").files(1), "1 arquivo");
  assert.equal(messagesFor("pt-BR").files(2), "2 arquivos");
  assert.equal(messagesFor("en").files(1), "1 file");
  assert.equal(messagesFor("en").files(2), "2 files");
  assert.equal(messagesFor("ja").files(2), "2ファイル");
  assert.equal(messagesFor("zh-CN").hours(3), "剩余约 3 小时");
  assert.equal(messagesFor("de").minutes(1), "~1 Minute verbleibend");
  assert.equal(messagesFor("fr").seconds(4), "~4 secondes restantes");
});
