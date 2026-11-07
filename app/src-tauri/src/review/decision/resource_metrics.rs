//! Optional process measurements. Linux samples procfs; unsupported platforms return None.
use std::time::Duration;
use tokio::task::JoinHandle;

#[derive(Default)]
pub struct Sample {
    pub cpu_ms: Option<u64>,
    pub peak_rss_bytes: Option<u64>,
}

pub struct Sampler {
    stop: std::sync::mpsc::SyncSender<()>,
    task: JoinHandle<Sample>,
}

impl Sampler {
    pub fn start(ids: Option<Vec<u32>>) -> Option<Self> {
        let ids = ids?;
        if ids.is_empty() {
            return None;
        }
        let (stop, stopped) = std::sync::mpsc::sync_channel(1);
        let task = tokio::task::spawn_blocking(move || {
            let mut sampler = PlatformSampler::new(ids);
            sampler.sample();
            loop {
                match stopped.recv_timeout(Duration::from_millis(50)) {
                    Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        sampler.sample();
                        return sampler.finish();
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => sampler.sample(),
                }
            }
        });
        Some(Self { stop, task })
    }

    pub async fn finish(self) -> Sample {
        let _ = self.stop.send(());
        self.task.await.unwrap_or_default()
    }
}

#[cfg(target_os = "linux")]
struct PlatformSampler {
    ids: Vec<u32>,
    identities: Option<Vec<u64>>,
    ticks_per_second: Option<u64>,
    last_cpu_ms: Option<u64>,
    peak_rss_bytes: Option<u64>,
}

#[cfg(target_os = "linux")]
impl PlatformSampler {
    fn new(ids: Vec<u32>) -> Self {
        // SAFETY: sysconf with _SC_CLK_TCK has no pointer arguments or side effects.
        let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        Self {
            ids,
            identities: None,
            ticks_per_second: u64::try_from(ticks).ok().filter(|ticks| *ticks > 0),
            last_cpu_ms: None,
            peak_rss_bytes: None,
        }
    }

    fn sample(&mut self) {
        let Some(rows) = self
            .ids
            .iter()
            .map(|id| read_process(*id))
            .collect::<Option<Vec<_>>>()
        else {
            self.last_cpu_ms = None;
            return;
        };
        let identities: Vec<_> = rows.iter().map(|r| r.0).collect();
        if self
            .identities
            .as_ref()
            .is_some_and(|old| old != &identities)
        {
            self.last_cpu_ms = None;
            return;
        }
        self.identities.get_or_insert(identities);
        let cpu_ticks = rows
            .iter()
            .try_fold(0_u64, |sum, row| sum.checked_add(row.1));
        self.last_cpu_ms = cpu_ticks.and_then(|ticks| {
            let hz = self.ticks_per_second?;
            u64::try_from(u128::from(ticks) * 1000 / u128::from(hz)).ok()
        });
        if let Some(rss) = rows
            .iter()
            .try_fold(0_u64, |sum, row| sum.checked_add(row.2))
        {
            self.peak_rss_bytes = Some(self.peak_rss_bytes.unwrap_or(0).max(rss));
        }
    }

    fn finish(self) -> Sample {
        Sample {
            cpu_ms: self.last_cpu_ms,
            peak_rss_bytes: self.peak_rss_bytes,
        }
    }
}

#[cfg(target_os = "linux")]
fn read_process(pid: u32) -> Option<(u64, u64, u64)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The parenthesized command may contain spaces or ')' characters.
    let fields: Vec<_> = stat
        .get(stat.rfind(')')? + 1..)?
        .split_whitespace()
        .collect();
    let ticks = fields
        .get(11)?
        .parse::<u64>()
        .ok()?
        .checked_add(fields.get(12)?.parse().ok()?)?;
    let identity = fields.get(19)?.parse().ok()?;
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let rss_kib = status.lines().find_map(|line| {
        line.strip_prefix("VmRSS:")?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()
    })?;
    Some((identity, ticks, rss_kib.checked_mul(1024)?))
}

#[cfg(not(target_os = "linux"))]
struct PlatformSampler;
#[cfg(not(target_os = "linux"))]
impl PlatformSampler {
    fn new(_: Vec<u32>) -> Self {
        Self
    }
    fn sample(&mut self) {}
    fn finish(self) -> Sample {
        Sample::default()
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn current_process_has_consistent_identity_and_rss() {
        let first = super::read_process(std::process::id()).unwrap();
        let second = super::read_process(std::process::id()).unwrap();
        assert_eq!(first.0, second.0);
        assert!(second.1 >= first.1);
        assert!(second.2 > 0);
    }

    #[tokio::test]
    async fn sampler_reports_measured_current_process() {
        let sampler = super::Sampler::start(Some(vec![std::process::id()])).unwrap();
        let sample = sampler.finish().await;
        assert!(sample.cpu_ms.is_some());
        assert!(sample.peak_rss_bytes.unwrap() > 0);
    }

    #[test]
    fn incomplete_process_group_does_not_invent_aggregate_cpu() {
        let mut sampler = super::PlatformSampler::new(vec![std::process::id(), u32::MAX]);
        sampler.sample();
        let sample = sampler.finish();
        assert!(sample.cpu_ms.is_none());
        assert!(sample.peak_rss_bytes.is_none());
    }
}
