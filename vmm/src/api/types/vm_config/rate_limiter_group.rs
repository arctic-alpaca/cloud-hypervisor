use option_parser::OptionParser;
use serde::{Deserialize, Serialize};
use virtio_devices::{RateLimiterConfig, TokenBucketConfig};

use crate::config::Error;
use crate::vm_config;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct RateLimiterGroupConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub rate_limiter_config: RateLimiterConfig,
}

impl From<RateLimiterGroupConfig> for vm_config::RateLimiterGroupConfig {
    fn from(value: RateLimiterGroupConfig) -> Self {
        Self {
            id: value.id,
            rate_limiter_config: value.rate_limiter_config,
        }
    }
}

impl From<&vm_config::RateLimiterGroupConfig> for RateLimiterGroupConfig {
    fn from(value: &vm_config::RateLimiterGroupConfig) -> Self {
        Self {
            id: value.id.clone(),
            rate_limiter_config: value.rate_limiter_config,
        }
    }
}

impl RateLimiterGroupConfig {
    pub const SYNTAX: &'static str = "Rate Limit Group parameters \
        \"bw_size=<bytes>,bw_one_time_burst=<bytes>,bw_refill_time=<ms>,\
        ops_size=<io_ops>,ops_one_time_burst=<io_ops>,ops_refill_time=<ms>,\
        id=<device_id>\"";

    pub fn parse(rate_limit_group: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser
            .add("bw_size")
            .add("bw_one_time_burst")
            .add("bw_refill_time")
            .add("ops_size")
            .add("ops_one_time_burst")
            .add("ops_refill_time")
            .add("id");
        parser
            .parse(rate_limit_group)
            .map_err(Error::ParseRateLimiterGroup)?;

        let id = parser.get("id").unwrap_or_default();
        let bw_size = parser
            .convert("bw_size")
            .map_err(Error::ParseRateLimiterGroup)?
            .unwrap_or_default();
        let bw_one_time_burst = parser
            .convert("bw_one_time_burst")
            .map_err(Error::ParseRateLimiterGroup)?
            .unwrap_or_default();
        let bw_refill_time = parser
            .convert("bw_refill_time")
            .map_err(Error::ParseRateLimiterGroup)?
            .unwrap_or_default();
        let ops_size = parser
            .convert("ops_size")
            .map_err(Error::ParseRateLimiterGroup)?
            .unwrap_or_default();
        let ops_one_time_burst = parser
            .convert("ops_one_time_burst")
            .map_err(Error::ParseRateLimiterGroup)?
            .unwrap_or_default();
        let ops_refill_time = parser
            .convert("ops_refill_time")
            .map_err(Error::ParseRateLimiterGroup)?
            .unwrap_or_default();

        let bw_tb_config = if bw_size != 0 && bw_refill_time != 0 {
            Some(TokenBucketConfig {
                size: bw_size,
                one_time_burst: Some(bw_one_time_burst),
                refill_time: bw_refill_time,
            })
        } else {
            None
        };
        let ops_tb_config = if ops_size != 0 && ops_refill_time != 0 {
            Some(TokenBucketConfig {
                size: ops_size,
                one_time_burst: Some(ops_one_time_burst),
                refill_time: ops_refill_time,
            })
        } else {
            None
        };

        Ok(RateLimiterGroupConfig {
            id,
            rate_limiter_config: RateLimiterConfig {
                bandwidth: bw_tb_config,
                ops: ops_tb_config,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_group_parsing() -> Result<(), Error> {
        assert_eq!(
            RateLimiterGroupConfig::parse("id=group0,bw_size=1000,bw_refill_time=100")?,
            RateLimiterGroupConfig {
                id: "group0".to_string(),
                rate_limiter_config: RateLimiterConfig {
                    bandwidth: Some(TokenBucketConfig {
                        size: 1000,
                        one_time_burst: Some(0),
                        refill_time: 100,
                    }),
                    ops: None,
                }
            }
        );
        assert_eq!(
            RateLimiterGroupConfig::parse("id=group0,ops_size=1000,ops_refill_time=100")?,
            RateLimiterGroupConfig {
                id: "group0".to_string(),
                rate_limiter_config: RateLimiterConfig {
                    bandwidth: None,
                    ops: Some(TokenBucketConfig {
                        size: 1000,
                        one_time_burst: Some(0),
                        refill_time: 100,
                    }),
                }
            }
        );
        Ok(())
    }
}
