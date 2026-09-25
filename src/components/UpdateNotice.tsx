import { emit, listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type { Update } from "@tauri-apps/plugin-updater";
import { messages } from "../i18n/index.ts";
import { downloadAndInstall, failureKind, lookForUpdate, restartApp } from "../services/updates";

type Status =
  | "idle"
  | "checking"
  | "none"
  | "available"
  | "downloading"
  | "installing"
  | "installed"
  | "check-error"
  | "install-error"
  | "signature-error";

const BUSY_EVENT = "crypto-busy";

export function UpdateNotice({ autoCheck, busy, showButton }: { autoCheck: boolean; busy: boolean; showButton: boolean }) {
  const [status, setStatus] = useState<Status>("idle");
  const [remoteBusy, setRemoteBusy] = useState(false);
  const [update, setUpdate] = useState<Update | null>(null);
  const blocked = busy || remoteBusy;

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<boolean>(BUSY_EVENT, (event) => setRemoteBusy(event.payload)).then((stop) => {
      unlisten = stop;
    }).catch(() => undefined);
    void emit("crypto-busy-ask").catch(() => undefined);
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    if (!autoCheck) return;
    void runCheck(false);
  }, [autoCheck]);

  async function runCheck(manual: boolean) {
    if (status === "downloading" || status === "installing") return;
    setStatus("checking");
    try {
      const found = await lookForUpdate();
      setUpdate(found);
      setStatus(found ? "available" : manual ? "none" : "idle");
    } catch (error) {
      setUpdate(null);
      const kind = failureKind(error, "check");
      if (kind === "signature") {
        setStatus("signature-error");
        return;
      }
      setStatus(manual ? "check-error" : "idle");
    }
  }

  async function install() {
    if (!update || blocked || status === "downloading" || status === "installing") return;
    try {
      await downloadAndInstall(update, (phase) => setStatus(phase === "downloading" ? "downloading" : "installing"));
      setStatus("installed");
    } catch (error) {
      setStatus(failureKind(error, "install") === "signature" ? "signature-error" : "install-error");
    }
  }

  const text = statusText(status);
  const offer = status === "available" || status === "downloading" || status === "installing" || status === "installed" || status === "signature-error" || status === "install-error" || status === "check-error" || status === "none";

  return (
    <>
      {showButton && status !== "downloading" && status !== "installing" && (
        <button className="about-links-btn" type="button" onClick={() => void runCheck(true)}>
          {status === "checking" ? messages.checkingUpdates : messages.checkForUpdates}
        </button>
      )}
      {offer && text && (
        <div className="update-panel" role="status">
          <p>{text}</p>
          {status === "available" && blocked && <p>{messages.updateWhileBusy}</p>}
          {status === "available" && (
            <div className="row">
              <button className="btn quiet" type="button" onClick={() => setStatus("idle")}>{messages.updateLater}</button>
              <button className="btn primary" type="button" disabled={blocked} onClick={() => void install()}>{messages.updateNow}</button>
            </div>
          )}
          {status === "installed" && (
            <button className="btn primary" type="button" disabled={blocked} onClick={() => void restartApp().catch(() => setStatus("install-error"))}>
              {messages.restartToFinish}
            </button>
          )}
        </div>
      )}
    </>
  );
}

export function publishCryptoBusy(busy: boolean) {
  void emit(BUSY_EVENT, busy).catch(() => undefined);
}

function statusText(status: Status): string {
  switch (status) {
    case "checking":
      return messages.checkingUpdates;
    case "none":
      return messages.noUpdateAvailable;
    case "available":
      return messages.updateAvailable;
    case "downloading":
      return messages.downloadingUpdate;
    case "installing":
      return messages.installingUpdate;
    case "installed":
      return messages.updateInstalled;
    case "check-error":
      return messages.updateCheckError;
    case "install-error":
      return messages.updateInstallError;
    case "signature-error":
      return messages.updateSignatureError;
    default:
      return "";
  }
}
