use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default)]
pub struct AudioStats {
    pub bitrate: Option<u64>,
    pub max_bitrate: Option<u64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
}

impl AudioStats {
    pub fn maximum(&self) -> Option<bool> {
        Some(self.bitrate? >= self.max_bitrate?)
    }

    pub fn status(&self) -> &'static str {
        match self.maximum() {
            Some(true) => "Máx. AAC",
            Some(false) => "Hay AAC superior",
            None => "Máxima sin verificar",
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatsOptions {
    pub enabled: bool,
    pub bitrate: bool,
    pub sample_rate: bool,
    pub channels: bool,
    pub maximum: bool,
    pub buffer: bool,
}

impl Default for StatsOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            bitrate: true,
            sample_rate: false,
            channels: false,
            maximum: true,
            buffer: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_measurements_never_claim_maximum() {
        for stats in [
            AudioStats::default(),
            AudioStats {
                bitrate: Some(128_000),
                ..Default::default()
            },
        ] {
            assert_eq!(stats.maximum(), None);
        }
    }

    #[test]
    fn compares_the_playing_format_with_the_reported_maximum() {
        let mut stats = AudioStats {
            bitrate: Some(128_000),
            max_bitrate: Some(256_000),
            ..Default::default()
        };
        assert_eq!(stats.maximum(), Some(false));
        stats.max_bitrate = Some(128_000);
        assert_eq!(stats.maximum(), Some(true));
    }

    #[test]
    fn old_settings_get_visible_quality_defaults() {
        let options: StatsOptions = serde_json::from_str("{}").unwrap();
        assert!(options.enabled && options.bitrate && options.maximum);
    }
}
