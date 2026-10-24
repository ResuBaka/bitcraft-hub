//! Opt-in startup diagnostics. Linux reports RSS and lifetime high-water RSS;
//! neither is a measurement of live Rust allocations.
use std::time::Instant;

pub(crate) struct StartupMemory {
    started: Instant,
}

impl StartupMemory {
    pub(crate) fn new() -> Self {
        Self {
            started: Instant::now(),
        }
    }

    pub(crate) fn snapshot(&self, stage: &str) {
        let elapsed = self.started.elapsed().as_secs_f64();
        match std::fs::read_to_string("/proc/self/status") {
            Ok(status) => {
                let (rss, peak) = parse_status(&status);
                eprintln!(
                    "startup_memory stage={stage} elapsed_s={elapsed:.3} rss_kib={rss:?} peak_rss_kib={peak:?}"
                );
            }
            Err(err) => eprintln!(
                "startup_memory stage={stage} elapsed_s={elapsed:.3} memory_unavailable={err}"
            ),
        }
    }
}

fn parse_status(status: &str) -> (Option<u64>, Option<u64>) {
    let value = |key: &str| {
        status.lines().find_map(|line| {
            line.strip_prefix(key)?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
    };
    (value("VmRSS:"), value("VmHWM:"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn diagnostic_is_explicit_and_has_a_configurable_settling_delay() {
        let cli = crate::Cli::try_parse_from(["api", "profile-init"]).unwrap();
        assert_eq!(
            cli.command,
            crate::Commands::ProfileInit { settle_seconds: 5 }
        );
        let cli =
            crate::Cli::try_parse_from(["api", "profile-init", "--settle-seconds", "0"]).unwrap();
        assert_eq!(
            cli.command,
            crate::Commands::ProfileInit { settle_seconds: 0 }
        );
        assert!(crate::Cli::try_parse_from(["api", "serve", "--settle-seconds", "0"]).is_err());
    }

    #[test]
    fn reads_rss_and_high_water_in_kib() {
        assert_eq!(
            parse_status("Name:\tapi\nVmHWM:\t2048 kB\nVmRSS:\t1024 kB\n"),
            (Some(1024), Some(2048))
        );
    }

    #[test]
    fn missing_or_malformed_values_are_unavailable() {
        assert_eq!(parse_status("VmRSS: invalid kB\n"), (None, None));
    }
}
