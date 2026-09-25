import { useEffect, useState } from "react";
import { locale, messages } from "../i18n/index.ts";
import { publishCryptoBusy } from "../components/UpdateNotice";
import { asAppError } from "../services/crypto";
import {
  MAX_TEXT_MESSAGE_LENGTH,
  characterCount,
  clampToMessageLength,
  decryptText,
  encryptText,
  looksLikeOpenPgpMessage,
} from "../services/textCrypto";

type Intent = "encrypt" | "decrypt";

export function TextPage({ onFiles }: { onFiles: () => void }) {
  const [intent, setIntent] = useState<Intent>("encrypt");
  const [message, setMessage] = useState("");
  const [password, setPassword] = useState("");
  const [visible, setVisible] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const count = characterCount(message);
  const armored = looksLikeOpenPgpMessage(message);
  const ready = !busy
    && password.length > 0
    && message.trim().length > 0
    && (intent === "decrypt" || (!armored && count <= MAX_TEXT_MESSAGE_LENGTH));

  useEffect(() => {
    publishCryptoBusy(busy);
    return () => publishCryptoBusy(false);
  }, [busy]);

  function selectIntent(next: Intent) {
    setIntent(next);
    setNotice(null);
    if (next === "encrypt" && !looksLikeOpenPgpMessage(message)) {
      setMessage(clampToMessageLength(message));
    }
  }

  function reset() {
    setMessage("");
    setPassword("");
    setResult(null);
    setNotice(null);
    setCopied(false);
    setIntent("encrypt");
  }

  async function run() {
    if (!ready) return;
    setBusy(true);
    setNotice(null);
    setCopied(false);
    try {
      const output = intent === "encrypt"
        ? await encryptText(message, password)
        : await decryptText(message, password);
      setPassword("");
      setResult(output);
    } catch (error) {
      const appError = asAppError(error);
      setNotice(messages.errors[appError.code] ?? (intent === "encrypt" ? messages.textEncryptError : messages.textDecryptError));
    } finally {
      setBusy(false);
    }
  }

  async function copy(value: string) {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
    } catch {
      setNotice(messages.textEncryptError);
    }
  }

  if (result !== null) {
    const title = intent === "encrypt" ? messages.encryptedMessage : messages.decryptedMessage;
    return (
      <div className="text-flow">
        <button className="text-back" type="button" onClick={onFiles}>{messages.back}</button>
        <h1>{title}</h1>
        <textarea className="text-box" readOnly value={result} />
        <div className="row">
          <button className="btn primary" type="button" onClick={() => void copy(result)}>{copied ? messages.copied : messages.copy}</button>
          <button className="btn quiet" type="button" onClick={reset}>{messages.newMessage}</button>
        </div>
      </div>
    );
  }

  return (
    <form className="text-flow" onSubmit={(event) => { event.preventDefault(); void run(); }}>
      <div className="text-top">
        <button className="text-back" type="button" onClick={onFiles}>{messages.back}</button>
        <div className="intent-switch" role="tablist">
          <button type="button" aria-pressed={intent === "encrypt"} onClick={() => selectIntent("encrypt")}>{messages.encrypt}</button>
          <button type="button" aria-pressed={intent === "decrypt"} onClick={() => selectIntent("decrypt")}>{messages.decrypt}</button>
        </div>
      </div>
      <label className="text-message">
        {intent === "encrypt" ? messages.messageLabel : messages.encryptedMessage}
        <textarea
          className="text-box"
          value={message}
          placeholder={intent === "encrypt" ? messages.messagePlaceholder : messages.armoredPlaceholder}
          disabled={busy}
          onChange={(event) => {
            const raw = event.target.value;
            const keepArmor = intent === "encrypt" && looksLikeOpenPgpMessage(raw);
            setMessage(intent === "encrypt" && !keepArmor ? clampToMessageLength(raw) : raw);
            setNotice(null);
          }}
        />
      </label>
      {intent === "encrypt" && !armored && (
        <p className="meta">{count.toLocaleString(locale)} / {MAX_TEXT_MESSAGE_LENGTH.toLocaleString(locale)}</p>
      )}
      {intent === "encrypt" && armored && (
        <button className="text-hint" type="button" onClick={() => setIntent("decrypt")}>{messages.pgpHint}</button>
      )}
      <label>
        {messages.password}
        <span className="field">
          <input
            type={visible ? "text" : "password"}
            value={password}
            autoComplete="new-password"
            placeholder={messages.passwordPlaceholder}
            disabled={busy}
            onChange={(event) => setPassword(event.target.value)}
          />
          <button className="ghost" type="button" onClick={() => setVisible((value) => !value)}>{visible ? messages.hide : messages.show}</button>
        </span>
      </label>
      {notice && <p className="warning">{notice}</p>}
      <button className="btn primary" type="submit" disabled={!ready}>
        {busy ? (intent === "encrypt" ? messages.textEncrypting : messages.textDecrypting) : intent === "encrypt" ? messages.encrypt : messages.decrypt}
      </button>
    </form>
  );
}
