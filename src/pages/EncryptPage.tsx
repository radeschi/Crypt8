import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useState } from "react";
import { APP_NAME, APP_WEBSITE, APP_WEBSITE_LABEL } from "../branding";
import { DocumentMark, ScannerBand } from "../components/DocumentMark";
import { publishCryptoBusy, UpdateNotice } from "../components/UpdateNotice";
import { TextPage } from "./TextPage";
import { locale, messages } from "../i18n/index.ts";
import { useFileDrop } from "../hooks/useFileDrop";
import {
  asAppError,
  cancelEncrypt,
  classifyPlaintext,
  decryptFile,
  encryptFile,
  folderActionLabel,
  inspectEntries,
  onEncryptProgress,
  pathExists,
  prepareOpenDirectory,
  prepareOpenTarget,
  suggestedOutputFor,
  suggestedPlaintextName,
  verifyDecryptPassword,
} from "../services/crypto";
import { formatBytes, formatRate, formatRemaining } from "../services/format";
import { passwordIssue } from "../services/password";
import type { EncryptResult, ProgressEvent, SelectionInfo } from "../types/progress";

type Phase =
  | "pick"
  | "encrypt"
  | "encrypting"
  | "encrypt-done"
  | "decrypt-ask"
  | "decrypt-password"
  | "decrypt-choice"
  | "decrypt-place"
  | "decrypting"
  | "decrypt-done";

export function EncryptPage() {
  const [phase, setPhase] = useState<Phase>("pick");
  const [file, setFile] = useState<SelectionInfo | null>(null);
  const [archive, setArchive] = useState(false);
  const [outputPath, setOutputPath] = useState("");
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [heldPassword, setHeldPassword] = useState("");
  const [visible, setVisible] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [replaceAsk, setReplaceAsk] = useState(false);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [operationId, setOperationId] = useState<string | null>(null);
  const [result, setResult] = useState<EncryptResult | null>(null);
  const [revealLabel, setRevealLabel] = useState("Abrir pasta");
  const [busy, setBusy] = useState(false);
  const [openedTemp, setOpenedTemp] = useState(false);
  const [mode, setMode] = useState<"files" | "text">("files");

  useEffect(() => {
    void folderActionLabel()
      .then((key) => setRevealLabel(messages.reveal[key as "finder" | "explorer" | "folder"] ?? messages.reveal.folder))
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void onEncryptProgress((event) => {
      setProgress((current) => {
        if (operationId && event.operationId !== operationId && current) {
          return current;
        }
        return event;
      });
    }).then((stop) => {
      unlisten = stop;
    }).catch(() => undefined);
    return () => unlisten?.();
  }, [operationId]);

  const chooseFile = useCallback(async (paths: string[]) => {
    setNotice(null);
    setReplaceAsk(false);
    setPassword("");
    setConfirmation("");
    setHeldPassword("");
    setArchive(false);
    try {
      const info = await inspectEntries(paths);
      setFile(info);
      if (info.intent === "decrypt") {
        setPhase("decrypt-ask");
        return;
      }
      setOutputPath(await suggestedOutputFor(info.paths));
      setPhase("encrypt");
    } catch (error) {
      setNotice(explain(error));
    }
  }, []);

  const rejectDrop = useCallback((message: string) => setNotice(message), []);
  const dragging = useFileDrop({
    enabled: mode === "files" && (phase === "pick" || phase === "encrypt" || phase === "decrypt-ask"),
    onFile: (paths) => void chooseFile(paths),
    onReject: rejectDrop,
  });

  const reset = useCallback(() => {
    setPhase("pick");
    setFile(null);
    setOutputPath("");
    setPassword("");
    setConfirmation("");
    setHeldPassword("");
    setVisible(false);
    setNotice(null);
    setReplaceAsk(false);
    setProgress(null);
    setOperationId(null);
    setResult(null);
    setOpenedTemp(false);
    setArchive(false);
  }, []);

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      if (phase === "encrypting" || phase === "decrypting") {
        if (operationId) void cancelEncrypt(operationId);
        return;
      }
      if (phase !== "pick") reset();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [operationId, phase, reset]);

  async function browse() {
    const selected = await open({ multiple: true, directory: false });
    if (Array.isArray(selected) && selected.length > 0) {
      await chooseFile(selected);
    } else if (typeof selected === "string") {
      await chooseFile([selected]);
    }
  }

  async function startEncrypt(overwrite: boolean) {
    if (!file) return;
    const issue = passwordIssue(password, confirmation);
    if (issue === "empty") {
      setNotice(messages.emptyPassword);
      return;
    }
    if (issue === "mismatch") {
      setNotice(messages.mismatch);
      return;
    }
    if (!overwrite && (await pathExists(outputPath))) {
      setReplaceAsk(true);
      return;
    }
    const id = crypto.randomUUID();
    const secret = password;
    setPassword("");
    setConfirmation("");
    setOperationId(id);
    setNotice(null);
    setReplaceAsk(false);
    setProgress(emptyProgress(id, file.sizeBytes));
    setPhase("encrypting");
    try {
      const encrypted = await encryptFile({
        operationId: id,
        inputPaths: file.paths,
        outputPath,
        password: secret,
        overwrite,
      });
      setResult(encrypted);
      setPhase("encrypt-done");
    } catch (error) {
      fail(error, "encrypt");
    }
  }

  async function confirmPassword() {
    if (!file || password.length === 0) {
      setNotice(messages.emptyPassword);
      return;
    }
    setBusy(true);
    setNotice(null);
    const secret = password;
    try {
      await verifyDecryptPassword(file.paths[0], secret);
      const packed = await classifyPlaintext(file.paths[0], secret);
      setArchive(packed);
      setHeldPassword(secret);
      setPassword("");
      setPhase(packed ? "decrypt-place" : "decrypt-choice");
    } catch (error) {
      setNotice(explain(error));
    } finally {
      setBusy(false);
    }
  }

  async function runDecrypt(target: string, overwrite: boolean, restrictPermissions: boolean, extract = archive) {
    if (!file) return;
    const id = crypto.randomUUID();
    setOperationId(id);
    setNotice(null);
    setProgress(emptyProgress(id, file.sizeBytes));
    setPhase("decrypting");
    try {
      const decrypted = await decryptFile({
        operationId: id,
        inputPath: file.paths[0],
        outputPath: target,
        password: heldPassword,
        overwrite,
        restrictPermissions,
        extract,
      });
      setHeldPassword("");
      setResult(decrypted);
      setOpenedTemp(restrictPermissions);
      if (restrictPermissions && !extract) {
        await openPath(decrypted.outputPath);
      }
      if (restrictPermissions && extract) {
        await revealItemInDir(decrypted.outputPath);
      }
      setPhase("decrypt-done");
    } catch (error) {
      setHeldPassword("");
      fail(error, "decrypt-password");
    }
  }

  async function openDecrypted() {
    if (!file) return;
    try {
      const target = archive ? await prepareOpenDirectory() : await prepareOpenTarget(file.paths[0]);
      await runDecrypt(target, true, true, archive);
    } catch (error) {
      setNotice(explain(error));
    }
  }

  async function saveDecrypted() {
    if (!file) return;
    if (archive) {
      const target = await open({ directory: true, multiple: false });
      if (typeof target !== "string") return;
      await runDecrypt(target, true, false, true);
      return;
    }
    const suggested = await suggestedPlaintextName(file.paths[0]);
    const target = await save({ defaultPath: suggested });
    if (typeof target !== "string") return;
    await runDecrypt(target, true, false, false);
  }

  function fail(error: unknown, back: Phase) {
    const appError = asAppError(error);
    setNotice(messages.errors[appError.code] ?? appError.message);
    if (import.meta.env.DEV && appError.detail) {
      console.error(appError.detail);
    }
    setPhase(back);
  }

  const issue = passwordIssue(password, confirmation);
  const working = phase === "encrypting" || phase === "decrypting";

  useEffect(() => {
    publishCryptoBusy(working);
    let unlisten: (() => void) | undefined;
    void listen("crypto-busy-ask", () => publishCryptoBusy(working)).then((stop) => {
      unlisten = stop;
    }).catch(() => undefined);
    return () => unlisten?.();
  }, [working]);
  const scanMode = phase === "decrypting" ? "decrypt" : working ? "encrypt" : "idle";
  const stats = progress
    ? [ `${Math.round(progress.percentage)}%`, formatRate(progress.bytesPerSecond, locale), formatRemaining(progress.estimatedRemainingSeconds, progress.status, messages) ]
        .filter(Boolean)
        .join(" · ")
    : "";

  return (
    <main className={dragging ? "app dragging" : "app"}>
      <header className="chrome" data-tauri-drag-region>
        <span className="wordmark" data-tauri-drag-region>{APP_NAME}</span>
      </header>
      <section className={dragging ? "stage hot" : mode === "text" ? "stage text-mode" : "stage"} key={mode === "text" ? "text" : phase}>
        {mode === "text" ? (
          <TextPage onFiles={() => setMode("files")} />
        ) : phase === "pick" && (
          <>
            <div className="mode-switch">
              <button type="button" aria-pressed="true">{messages.filesMode}</button>
              <button type="button" aria-pressed="false" onClick={() => setMode("text")}>{messages.textMode}</button>
            </div>
            <div className="band">
              <svg className="arrow" viewBox="0 0 18 18" aria-hidden="true">
                <path d="M9 2v12M4 10l5 5 5-5" fill="none" stroke="currentColor" strokeWidth="1.6" />
              </svg>
              <span className="caption">{dragging ? messages.dropRelease : messages.dropHere}</span>
              <button type="button" onClick={() => void browse()}>{messages.chooseFile}</button>
            </div>
          </>
        )}

        {phase === "encrypt" && file && (
          <>
            <DocumentMark mode="idle" />
            <p className="file-name">{selectionLabel(file)}</p>
            <p className="meta">{formatBytes(file.sizeBytes, locale)}</p>
            <form className="form" onSubmit={(event) => { event.preventDefault(); void startEncrypt(false); }}>
              <label>
                {messages.password}
                <span className="field">
                  <input aria-label={messages.password} type={visible ? "text" : "password"} value={password} autoComplete="new-password" onChange={(event) => setPassword(event.target.value)} />
                  <button className="ghost" type="button" onClick={() => setVisible((value) => !value)}>{visible ? messages.hide : messages.show}</button>
                </span>
              </label>
              <label>
                {messages.confirmPassword}
                <span className="field">
                  <input aria-label={messages.confirmPassword} type={visible ? "text" : "password"} value={confirmation} autoComplete="new-password" onChange={(event) => setConfirmation(event.target.value)} />
                </span>
              </label>
              {issue === "mismatch" && confirmation.length > 0 && <p className="warning">{messages.mismatch}</p>}
              {replaceAsk ? (
                <div className="row">
                  <button className="btn primary" type="button" onClick={() => void startEncrypt(true)}>{messages.replace}</button>
                  <button className="btn quiet" type="button" onClick={() => setReplaceAsk(false)}>{messages.keep}</button>
                </div>
              ) : (
                <div className="row">
                  <button className="btn quiet" type="button" onClick={reset}>{messages.cancel}</button>
                  <button className="btn primary" type="submit" disabled={issue !== null}>{messages.encrypt}</button>
                </div>
              )}
            </form>
          </>
        )}

        {phase === "decrypt-ask" && file && (
          <>
            <DocumentMark mode="decrypt" />
            <p className="file-name">{selectionLabel(file)}</p>
            <h1>{messages.decryptAsk}</h1>
            <div className="row">
              <button className="btn primary" type="button" onClick={() => { setNotice(null); setPhase("decrypt-password"); }}>{messages.decrypt}</button>
              <button className="btn quiet" type="button" onClick={reset}>{messages.cancel}</button>
            </div>
          </>
        )}

        {phase === "decrypt-password" && file && (
          <form className="form" onSubmit={(event) => { event.preventDefault(); void confirmPassword(); }}>
            <DocumentMark mode="decrypt" />
            <p className="file-name">{selectionLabel(file)}</p>
            <label>
              {messages.password}
              <span className="field">
                <input aria-label={messages.password} type={visible ? "text" : "password"} value={password} autoComplete="current-password" onChange={(event) => setPassword(event.target.value)} />
                <button className="ghost" type="button" onClick={() => setVisible((value) => !value)}>{visible ? messages.hide : messages.show}</button>
              </span>
            </label>
            <div className="row">
              <button className="btn quiet" type="button" onClick={reset}>{messages.cancel}</button>
              <button className="btn primary" type="submit" disabled={busy || password.length === 0}>{messages.decrypt}</button>
            </div>
          </form>
        )}

        {phase === "decrypt-choice" && file && (
          <>
            <DocumentMark mode="decrypt" />
            <p className="file-name">{selectionLabel(file)}</p>
            <h1>{messages.decrypt}</h1>
            <div className="row">
              <button className="btn primary" type="button" onClick={() => void openDecrypted()}>{messages.open}</button>
              <button className="btn quiet" type="button" onClick={() => void saveDecrypted()}>{messages.saveAs}</button>
            </div>
          </>
        )}

        {phase === "decrypt-place" && file && (
          <>
            <DocumentMark mode="decrypt" />
            <p className="file-name">{selectionLabel(file)}</p>
            <h1>{messages.chooseWhere}</h1>
            <div className="row">
              <button className="btn primary" type="button" onClick={() => void openDecrypted()}>{messages.open}</button>
              <button className="btn quiet" type="button" onClick={() => void saveDecrypted()}>{messages.saveAs}</button>
            </div>
          </>
        )}

        {working && file && progress && (
          <>
            <div className="doc-anchor">
              <div className="scan-clip">
                <ScannerBand />
              </div>
              <DocumentMark mode={scanMode} />
            </div>
            <p className="file-name">{selectionLabel(file)}</p>
            <h1>{phase === "decrypting" ? messages.decrypting : messages.encrypting}</h1>
            <p className="quiet">{stats}</p>
            <button className="btn quiet" type="button" onClick={() => operationId && void cancelEncrypt(operationId)}>{messages.cancel}</button>
          </>
        )}

        {phase === "encrypt-done" && result && (
          <>
            <DocumentMark mode="done" />
            <h1>{messages.encrypted}</h1>
            <p className="file-name">{result.outputName}</p>
            <div className="row">
              <button className="btn primary" type="button" onClick={() => void revealItemInDir(result.outputPath)}>{revealLabel}</button>
              <button className="btn quiet" type="button" onClick={reset}>{messages.another}</button>
            </div>
          </>
        )}

        {phase === "decrypt-done" && result && (
          <>
            <DocumentMark mode="done" />
            <h1>{archive ? messages.restored : openedTemp ? messages.opened : messages.saved}</h1>
            <p className="file-name">{result.outputName}</p>
            {openedTemp && !archive && <p className="hint">{messages.tempHint}</p>}
            <div className="row">
              {!openedTemp && (
                <button className="btn primary" type="button" onClick={() => void revealItemInDir(result.outputPath)}>{revealLabel}</button>
              )}
              <button className="btn quiet" type="button" onClick={reset}>{messages.again}</button>
            </div>
          </>
        )}

        {notice && !working && <p className="warning">{notice}</p>}
      </section>
      <a
        className="signature"
        href={APP_WEBSITE}
        onClick={(event) => {
          event.preventDefault();
          void openUrl(APP_WEBSITE);
        }}
      >
        {APP_WEBSITE_LABEL}
      </a>
      <UpdateNotice autoCheck busy={working} showButton={false} />
      <div className="progress-track" aria-hidden="true">
        <span style={{ width: working && progress ? `${progress.percentage}%` : "0%" }} />
      </div>
    </main>
  );
}

function explain(error: unknown): string {
  const appError = asAppError(error);
  return messages.errors[appError.code] ?? appError.message;
}

function selectionLabel(file: SelectionInfo): string {
  if (file.kind !== "multiple") return file.name;
  const parts = [
    file.fileCount > 0 ? messages.files(file.fileCount) : "",
    file.directoryCount > 0 ? messages.folders(file.directoryCount) : "",
  ].filter(Boolean);
  return parts.join(" · ") || messages.items(file.paths.length);
}

function emptyProgress(operationId: string, totalBytes: number): ProgressEvent {
  return {
    operationId,
    processedBytes: 0,
    totalBytes,
    percentage: 0,
    bytesPerSecond: 0,
    averageBytesPerSecond: 0,
    estimatedRemainingSeconds: null,
    status: "preparing",
  };
}
