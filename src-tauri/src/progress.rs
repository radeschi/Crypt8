use std::time::Duration;

use serde::Serialize;

/// Amostras mínimas antes de estimar o tempo restante.
const MIN_ELAPSED_SECS: f64 = 0.4;
const MIN_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProgressStatus {
    Preparing,
    Encrypting,
    Finalizing,
    Completed,
    /// Emitido quando a interface precisar distinguir o cancelamento do erro genérico.
    #[allow(dead_code)]
    Cancelled,
    #[allow(dead_code)]
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub operation_id: String,
    pub processed_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f64,
    pub bytes_per_second: f64,
    pub average_bytes_per_second: f64,
    pub estimated_remaining_seconds: Option<f64>,
    pub status: ProgressStatus,
}

#[derive(Debug, Clone)]
pub struct ProgressSnapshot {
    pub processed_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f64,
    pub bytes_per_second: f64,
    pub average_bytes_per_second: f64,
    pub estimated_remaining_seconds: Option<f64>,
    pub status: ProgressStatus,
}

impl ProgressSnapshot {
    pub fn into_event(self, operation_id: impl Into<String>) -> ProgressEvent {
        ProgressEvent {
            operation_id: operation_id.into(),
            processed_bytes: self.processed_bytes,
            total_bytes: self.total_bytes,
            percentage: self.percentage,
            bytes_per_second: self.bytes_per_second,
            average_bytes_per_second: self.average_bytes_per_second,
            estimated_remaining_seconds: self.estimated_remaining_seconds,
            status: self.status,
        }
    }
}

/// Estima velocidade e tempo restante a partir de bytes já lidos do arquivo original.
pub struct ProgressEstimator {
    total: u64,
    processed: u64,
    started: std::time::Instant,
    last_emit: std::time::Instant,
    window_started: std::time::Instant,
    window_bytes: u64,
    current_bps: f64,
    emit_every: Duration,
}

impl ProgressEstimator {
    pub fn new(total: u64, emit_every: Duration) -> Self {
        let now = std::time::Instant::now();
        Self {
            total,
            processed: 0,
            started: now,
            last_emit: now.checked_sub(emit_every).unwrap_or(now),
            window_started: now,
            window_bytes: 0,
            current_bps: 0.0,
            emit_every,
        }
    }

    pub fn snapshot(&self, status: ProgressStatus) -> ProgressSnapshot {
        let elapsed = self.started.elapsed().as_secs_f64();
        let average = if elapsed > 0.0 {
            self.processed as f64 / elapsed
        } else {
            0.0
        };
        let current = if self.current_bps > 0.0 {
            self.current_bps
        } else {
            average
        };
        ProgressSnapshot {
            processed_bytes: self.processed,
            total_bytes: self.total,
            percentage: percentage(self.processed, self.total),
            bytes_per_second: current,
            average_bytes_per_second: average,
            estimated_remaining_seconds: estimate_remaining(self.processed, self.total, elapsed),
            status,
        }
    }

    /// Atualiza a contagem. Devolve um evento quando o intervalo de emissão venceu.
    pub fn push(
        &mut self,
        processed: u64,
        status: ProgressStatus,
        force: bool,
    ) -> Option<ProgressSnapshot> {
        let now = std::time::Instant::now();
        let delta = processed.saturating_sub(self.processed);
        self.processed = processed;
        self.window_bytes = self.window_bytes.saturating_add(delta);

        let window = now.duration_since(self.window_started);
        if window >= Duration::from_millis(250) {
            let secs = window.as_secs_f64();
            if secs > 0.0 {
                self.current_bps = self.window_bytes as f64 / secs;
            }
            self.window_bytes = 0;
            self.window_started = now;
        }

        if force || now.duration_since(self.last_emit) >= self.emit_every {
            self.last_emit = now;
            Some(self.snapshot(status))
        } else {
            None
        }
    }
}

pub fn percentage(processed: u64, total: u64) -> f64 {
    if total == 0 {
        return 100.0;
    }
    ((processed as f64 / total as f64) * 100.0).clamp(0.0, 100.0)
}

/// `None` enquanto não houver amostra suficiente. Não promete precisão.
pub fn estimate_remaining(processed: u64, total: u64, elapsed_secs: f64) -> Option<f64> {
    if processed == 0 || elapsed_secs < MIN_ELAPSED_SECS || processed < MIN_BYTES {
        return None;
    }
    if processed >= total {
        return Some(0.0);
    }
    let average = processed as f64 / elapsed_secs;
    if average <= 0.0 {
        return None;
    }
    Some((total - processed) as f64 / average)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_estimate_until_there_is_enough_data() {
        assert_eq!(estimate_remaining(0, 1_000_000, 2.0), None);
        assert_eq!(estimate_remaining(1000, 1_000_000, 2.0), None);
        assert_eq!(estimate_remaining(MIN_BYTES, 2_000_000, 0.1), None);
    }

    #[test]
    fn estimates_remaining_from_average_speed() {
        let remaining = estimate_remaining(1_048_576, 2_097_152, 1.0).unwrap();
        assert!((remaining - 1.0).abs() < 0.01);
    }

    #[test]
    fn reports_zero_when_the_file_is_fully_read() {
        assert_eq!(estimate_remaining(MIN_BYTES, MIN_BYTES, 1.0), Some(0.0));
    }

    #[test]
    fn percentage_handles_empty_files() {
        assert_eq!(percentage(0, 0), 100.0);
        assert!((percentage(50, 200) - 25.0).abs() < 0.01);
    }
}
