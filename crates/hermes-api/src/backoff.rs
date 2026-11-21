//! Exponential reconnect backoff with jitter.

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Backoff {
    base: Duration,
    max: Duration,
    attempt: u32,
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new(Duration::from_millis(500), Duration::from_secs(30))
    }
}

impl Backoff {
    pub fn new(base: Duration, max: Duration) -> Self {
        Self { base, max, attempt: 0 }
    }

    /// The next delay. It doubles each call up to `max`, with up to 20 % jitter.
    pub fn next_delay(&mut self) -> Duration {
        let exp = self.attempt.min(16);
        self.attempt = self.attempt.saturating_add(1);
        let raw = self.base.saturating_mul(1u32 << exp).min(self.max);
        let jitter = rand::random::<f64>() * 0.2;
        raw.mul_f64(1.0 - jitter)
    }

    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_and_caps() {
        let mut b = Backoff::new(Duration::from_millis(100), Duration::from_secs(1));
        let d: Vec<_> = (0..8).map(|_| b.next_delay()).collect();
        assert!(d[0] <= Duration::from_millis(100));
        assert!(d[0] >= Duration::from_millis(80));
        assert!(d[3] >= Duration::from_millis(640));
        assert!(d.iter().all(|x| *x <= Duration::from_secs(1)));
        b.reset();
        assert_eq!(b.attempt(), 0);
    }
}
