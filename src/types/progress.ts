export type ProgressStatus =
  | "preparing"
  | "encrypting"
  | "finalizing"
  | "completed"
  | "cancelled"
  | "error";

export interface ProgressEvent {
  operationId: string;
  processedBytes: number;
  totalBytes: number;
  percentage: number;
  bytesPerSecond: number;
  averageBytesPerSecond: number;
  estimatedRemainingSeconds: number | null;
  status: ProgressStatus;
}

export interface FileInfo {
  path: string;
  name: string;
  extension: string;
  sizeBytes: number;
  intent: "encrypt" | "decrypt";
}

export interface SelectionInfo {
  paths: string[];
  name: string;
  kind: "file" | "directory" | "multiple";
  sizeBytes: number;
  fileCount: number;
  directoryCount: number;
  intent: "encrypt" | "decrypt";
}

export interface EncryptResult {
  outputPath: string;
  outputName: string;
  inputBytes: number;
  outputBytes: number;
}

export interface AppError {
  code: string;
  message: string;
  detail?: string;
}
