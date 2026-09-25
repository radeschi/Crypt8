import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useRef, useState } from "react";

interface UseFileDropOptions {
  enabled: boolean;
  onFile: (paths: string[]) => void;
  onReject: (message: string) => void;
}

export function useFileDrop({ enabled, onFile, onReject }: UseFileDropOptions): boolean {
  const [dragging, setDragging] = useState(false);
  const onFileRef = useRef(onFile);
  const onRejectRef = useRef(onReject);
  onFileRef.current = onFile;
  onRejectRef.current = onReject;

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;

    let webview;
    try {
      webview = getCurrentWebview();
    } catch {
      return;
    }

    void webview
      .onDragDropEvent((event) => {
        if (!enabled) {
          return;
        }
        const payload = event.payload;
        if (payload.type === "enter" || payload.type === "over") {
          setDragging(true);
          return;
        }
        if (payload.type === "leave") {
          setDragging(false);
          return;
        }
        setDragging(false);
        if (payload.paths.length === 0) {
          return;
        }
        onFileRef.current(payload.paths);
      })
      .then((stop) => {
        if (active) {
          unlisten = stop;
        } else {
          stop();
        }
      });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [enabled]);

  return dragging;
}
