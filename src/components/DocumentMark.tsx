interface DocumentMarkProps {
  mode: "idle" | "encrypt" | "decrypt" | "done";
}

export function DocumentMark({ mode }: DocumentMarkProps) {
  return (
    <div className={`paper ${mode === "done" ? "idle" : mode}`} aria-hidden="true">
      <span className="line" />
      <span className="line" />
      <span className="line" />
      <span className="line" />
      {mode === "done" && <span className="check">✓</span>}
    </div>
  );
}

export function ScannerBand() {
  return (
    <div className="scanner" aria-hidden="true">
      {Array.from({ length: 7 }, (_, index) => (
        <span
          key={index}
          className="dot"
          style={{ left: `${12 + index * 12}%`, animationDelay: `${index * 0.18}s` }}
        />
      ))}
    </div>
  );
}
