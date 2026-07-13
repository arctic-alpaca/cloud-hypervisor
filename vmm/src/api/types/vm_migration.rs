use std::num::{NonZeroU32, NonZeroU64};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use option_parser::{OptionParser, OptionParserError, Toggle};
use serde::{Deserialize, Serialize};

/// Memory transfer mode for a migration.
#[derive(Copy, Clone, Default, Deserialize, Serialize, Debug, PartialEq, Eq)]
pub enum MigrationMode {
    /// Transfer all guest memory before the destination resumes.
    #[default]
    Precopy,
    /// Resume the destination first and fault guest pages in on demand.
    /// This is an experimental mode. It uses a single connection even
    /// when parallel connections are configured. Pages are served on
    /// demand, but a background faulting mechanism also pulls in the
    /// remaining pages to speed up completion.
    Postcopy,
}

impl FromStr for MigrationMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "precopy" => Ok(MigrationMode::Precopy),
            "postcopy" => Ok(MigrationMode::Postcopy),
            _ => Err(format!("Invalid migration mode: {s}")),
        }
    }
}

#[derive(Clone, Deserialize, Serialize, Default, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct VmReceiveMigrationData {
    /// URL for the reception of migration state
    pub receiver_url: String,
    /// Directory containing the TLS server certificate (`server-cert.pem`),
    /// the TLS server key (`server-key.pem`), and the server's TLS root CA
    /// certificate (`ca-cert.pem`).
    ///
    /// If this is `Some`, the migration is instructed to use mTLS.
    #[serde(default)]
    pub tls_dir: Option<PathBuf>,
    /// Memory transfer mode.
    #[serde(default)]
    pub memory_mode: MigrationMode,
}

impl VmReceiveMigrationData {
    pub const SYNTAX: &'static str = "VM receive migration parameters \
        \"<receiver_url>\" or \"receiver_url=<url>[,tls_dir=<path>][,memory_mode=precopy|postcopy]\"";

    pub fn parse(migration: &str) -> Result<Self, OptionParserError> {
        let mut parser = OptionParser::new();
        parser.add("receiver_url").add("tls_dir").add("memory_mode");
        parser.parse(migration)?;

        let receiver_url = parser.get("receiver_url").ok_or_else(|| {
            OptionParserError::InvalidSyntax("receiver_url is required".to_string())
        })?;
        let tls_dir = parser
            .convert::<String>("tls_dir")?
            .map(|path| PathBuf::from(&path));
        let memory_mode = parser
            .convert::<MigrationMode>("memory_mode")?
            .unwrap_or_default();

        let data = Self {
            receiver_url,
            tls_dir,
            memory_mode,
        };

        Ok(data)
    }
}

#[derive(Copy, Clone, Default, Deserialize, Serialize, Debug, PartialEq, Eq)]
/// The migration timeout strategy.
///
/// This strategy describes the behavior of the migration when the target
/// downtime can't be reached in the given timeout.
pub enum TimeoutStrategy {
    #[default]
    /// Cancel the migration and keep the VM running on the source.
    Cancel,
    /// Ignore the timeout and migrate anyway.
    Ignore,
}

impl FromStr for TimeoutStrategy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cancel" => Ok(TimeoutStrategy::Cancel),
            "ignore" => Ok(TimeoutStrategy::Ignore),
            _ => Err(format!("Invalid timeout strategy: {s}")),
        }
    }
}

/// Configuration for an outgoing migration.
#[derive(Clone, Deserialize, Serialize, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct VmSendMigrationData {
    /// Migration destination, e.g. `tcp:<host>:<port>` or `unix:/path/to/socket`.
    pub destination_url: String,
    /// Send memory across socket without copying
    #[serde(default)]
    pub local: bool,
    /// The maximum downtime the migration aims for.
    ///
    /// Usually, on the order of a few hundred milliseconds.
    #[serde(default = "VmSendMigrationData::default_downtime_ms")]
    pub downtime_ms: NonZeroU64,
    /// The timeout for the migration, i.e., the maximum duration.
    #[serde(default = "VmSendMigrationData::default_timeout_s")]
    pub timeout_s: NonZeroU64,
    /// The timeout strategy for the migration.
    #[serde(default)]
    pub timeout_strategy: TimeoutStrategy,

    /// The number of parallel TCP connections for migration.
    ///
    /// Must be between 1 and `MAX_MIGRATION_CONNECTIONS` inclusive.
    #[serde(default = "VmSendMigrationData::default_connections")]
    pub connections: NonZeroU32,
    /// Directory containing the TLS client certificate (`client-cert.pem`),
    /// the TLS client key (`client-key.pem`), and the client's TLS root CA
    /// certificate (`ca-cert.pem`).
    ///
    /// If this is `Some`, the migration is instructed to use mTLS.
    #[serde(default)]
    pub tls_dir: Option<PathBuf>,
    /// Memory transfer mode.
    #[serde(default)]
    pub memory_mode: MigrationMode,
}

impl VmSendMigrationData {
    pub const SYNTAX: &'static str = "VM send migration parameters \
        \"destination_url=<url>[,local=on|off,\
        downtime_ms=<milliseconds>,timeout_s=<seconds>,\
        timeout_strategy=cancel|ignore,connections=<amount>,\
        tls_dir=<path>,memory_mode=precopy|postcopy]\"";

    // Same as QEMU.
    pub const DEFAULT_DOWNTIME: Duration = Duration::from_millis(300);
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60 * 60 /* one hour */);

    fn default_downtime_ms() -> NonZeroU64 {
        let ms_u64 = u64::try_from(Self::DEFAULT_DOWNTIME.as_millis()).unwrap();
        NonZeroU64::new(ms_u64).unwrap()
    }

    fn default_timeout_s() -> NonZeroU64 {
        NonZeroU64::new(Self::DEFAULT_TIMEOUT.as_secs()).unwrap()
    }

    // Use a single connection as default for backward compatibility.
    fn default_connections() -> NonZeroU32 {
        NonZeroU32::new(1).unwrap()
    }

    pub fn parse(migration: &str) -> Result<Self, OptionParserError> {
        let mut parser = OptionParser::new();
        parser
            .add("destination_url")
            .add("local")
            .add("downtime_ms")
            .add("timeout_s")
            .add("timeout_strategy")
            .add("connections")
            .add("tls_dir")
            .add("memory_mode");
        parser.parse(migration)?;

        let destination_url = parser.get("destination_url").ok_or_else(|| {
            OptionParserError::InvalidSyntax("destination_url is required".to_string())
        })?;
        let local = parser
            .convert::<Toggle>("local")?
            .unwrap_or(Toggle(false))
            .0;
        let downtime_ms = match parser.convert::<u64>("downtime_ms")? {
            Some(v) => NonZeroU64::new(v).ok_or_else(|| {
                OptionParserError::InvalidValue("downtime_ms must be non-zero".to_string())
            })?,
            None => Self::default_downtime_ms(),
        };
        let timeout_s = match parser.convert::<u64>("timeout_s")? {
            Some(v) => NonZeroU64::new(v).ok_or_else(|| {
                OptionParserError::InvalidValue("timeout_s must be non-zero".to_string())
            })?,
            None => Self::default_timeout_s(),
        };
        let timeout_strategy = parser.convert("timeout_strategy")?.unwrap_or_default();
        let connections = match parser.convert::<u32>("connections")? {
            Some(v) => NonZeroU32::new(v).ok_or_else(|| {
                OptionParserError::InvalidValue("connections must be non-zero".to_string())
            })?,
            None => Self::default_connections(),
        };
        let tls_dir = parser
            .convert::<String>("tls_dir")?
            .map(|path| PathBuf::from(&path));
        let memory_mode = parser
            .convert::<MigrationMode>("memory_mode")?
            .unwrap_or_default();

        let data = Self {
            destination_url,
            local,
            downtime_ms,
            timeout_s,
            timeout_strategy,
            connections,
            tls_dir,
            memory_mode,
        };

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::{env, fs, process};

    use super::*;

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = env::temp_dir().join(format!(
                "cloud-hypervisor-api-{name}-{}-{unique}",
                process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }

        fn add_file(&self, file_name: &str) {
            fs::write(self.path.join(file_name), b"test").unwrap();
        }

        fn add_receive_tls_files(&self) {
            self.add_file("ca-cert.pem");
            self.add_file("server-cert.pem");
            self.add_file("server-key.pem");
        }

        fn add_send_tls_files(&self) {
            self.add_file("ca-cert.pem");
            self.add_file("client-cert.pem");
            self.add_file("client-key.pem");
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_vm_receive_migration_data_parse() {
        let data = VmReceiveMigrationData::parse("receiver_url=tcp:192.168.1.1:8080").unwrap();
        assert_eq!(
            data,
            VmReceiveMigrationData {
                receiver_url: "tcp:192.168.1.1:8080".to_string(),
                tls_dir: None,
                memory_mode: MigrationMode::Precopy,
            }
        );

        let data = VmReceiveMigrationData::parse("receiver_url=tcp:[2001:db8::1]:8080").unwrap();
        assert_eq!(data.receiver_url, "tcp:[2001:db8::1]:8080");

        let data =
            VmReceiveMigrationData::parse("receiver_url=tcp:destination.example:8080").unwrap();
        assert_eq!(data.receiver_url, "tcp:destination.example:8080");

        let data = VmReceiveMigrationData::parse("receiver_url=unix:/tmp/ch=migrate.sock").unwrap();
        assert_eq!(data.receiver_url, "unix:/tmp/ch=migrate.sock");

        let tls_dir = TestDir::new("receive-tls");
        tls_dir.add_receive_tls_files();
        let tls_dir_path = tls_dir.path.clone();
        let data = VmReceiveMigrationData::parse(&format!(
            "receiver_url=tcp:192.168.1.1:8080,tls_dir={}",
            tls_dir_path.display()
        ))
        .unwrap();
        assert_eq!(
            data,
            VmReceiveMigrationData {
                receiver_url: "tcp:192.168.1.1:8080".to_string(),
                tls_dir: Some(tls_dir_path),
                memory_mode: MigrationMode::Precopy,
            }
        );

        // memory_mode defaults to precopy when not specified.
        let data = VmReceiveMigrationData::parse("receiver_url=tcp:127.0.0.1:1234").unwrap();
        assert_eq!(
            data,
            VmReceiveMigrationData {
                receiver_url: "tcp:127.0.0.1:1234".to_string(),
                tls_dir: None,
                memory_mode: MigrationMode::Precopy,
            }
        );

        // Explicit receiver_url with memory_mode=postcopy.
        let data =
            VmReceiveMigrationData::parse("receiver_url=unix:/tmp/sock,memory_mode=postcopy")
                .unwrap();
        assert_eq!(
            data,
            VmReceiveMigrationData {
                receiver_url: "unix:/tmp/sock".to_string(),
                tls_dir: None,
                memory_mode: MigrationMode::Postcopy,
            }
        );

        // Missing receiver_url in keyed form must fail.
        VmReceiveMigrationData::parse("memory_mode=postcopy").unwrap_err();
    }

    #[test]
    fn test_vm_send_migration_data_parse() {
        // Fully specified
        let data = VmSendMigrationData::parse(
            "destination_url=unix:/tmp/migrate.sock,local=on,downtime_ms=200,timeout_s=3600,timeout_strategy=cancel"
        ).expect("valid migration string should parse");
        assert_eq!(data.destination_url, "unix:/tmp/migrate.sock");
        assert!(data.local);
        assert_eq!(data.downtime_ms.get(), 200);
        assert_eq!(data.timeout_s.get(), 3600);
        assert_eq!(data.timeout_strategy, TimeoutStrategy::Cancel);
        assert_eq!(data.connections.get(), 1);

        // Defaults applied when optional fields are omitted
        let data = VmSendMigrationData::parse("destination_url=tcp:192.168.1.1:8080")
            .expect("minimal migration string should parse");
        assert_eq!(data.destination_url, "tcp:192.168.1.1:8080");
        assert!(!data.local);
        assert_eq!(data.downtime_ms, VmSendMigrationData::default_downtime_ms());
        assert_eq!(data.timeout_s, VmSendMigrationData::default_timeout_s());
        assert_eq!(data.timeout_strategy, TimeoutStrategy::default());
        assert_eq!(data.connections, VmSendMigrationData::default_connections());

        let data = VmSendMigrationData::parse("destination_url=tcp:[2001:db8::1]:8080")
            .expect("IPv6 migration string should parse");
        assert_eq!(data.destination_url, "tcp:[2001:db8::1]:8080");

        let data = VmSendMigrationData::parse("destination_url=tcp:destination.example:8080")
            .expect("hostname migration string should parse");
        assert_eq!(data.destination_url, "tcp:destination.example:8080");

        // Missing destination_url is an error
        VmSendMigrationData::parse("local=on,downtime_ms=200").unwrap_err();

        // Zero downtime_ms is rejected
        let _data =
            VmSendMigrationData::parse("destination_url=tcp:192.168.1.1:8080,downtime_ms=0")
                .expect_err("zero downtime_ms should be rejected");

        // Zero timeout_s is rejected
        let _data = VmSendMigrationData::parse("destination_url=unix:/tmp/sock,timeout_s=0")
            .expect_err("zero timeout_s should be rejected");

        // Zero connections is rejected
        let _data =
            VmSendMigrationData::parse("destination_url=tcp:192.168.1.1:8080,connections=0")
                .expect_err("zero connections should be rejected");

        // Unknown option is an error
        VmSendMigrationData::parse("destination_url=unix:/tmp/sock,unknown_field=foo").unwrap_err();

        // Invalid toggle value is an error
        VmSendMigrationData::parse("destination_url=unix:/tmp/sock,local=yes").unwrap_err();

        // Timeout strategy
        let _data = VmSendMigrationData::parse(
            "destination_url=tcp:192.168.1.1:8080,timeout_strategy=invalid",
        )
        .expect_err("invalid timeout strategy should be rejected");

        // Local migration requires a UNIX socket destination
        VmSendMigrationData::parse("destination_url=tcp:192.168.1.1:8080,local=yes").unwrap_err();

        // Local migration cannot use multiple connections
        VmSendMigrationData::parse("destination_url=unix:/tmp/sock,local=yes,connections=2")
            .unwrap_err();

        // Happy path with some defaults
        let data =
            VmSendMigrationData::parse("destination_url=tcp:192.168.1.1:8080,downtime_ms=150")
                .unwrap();
        assert_eq!(
            data,
            VmSendMigrationData {
                destination_url: "tcp:192.168.1.1:8080".to_string(),
                local: false,
                downtime_ms: NonZeroU64::new(150).unwrap(),
                timeout_s: VmSendMigrationData::default_timeout_s(),
                timeout_strategy: Default::default(),
                connections: VmSendMigrationData::default_connections(),
                tls_dir: None,
                memory_mode: MigrationMode::Precopy,
            }
        );

        // Happy path, fully specified
        let tls_dir = TestDir::new("send-tls");
        tls_dir.add_send_tls_files();
        let tls_dir_path = tls_dir.path.clone();
        let data =
            VmSendMigrationData::parse(&format!("destination_url=tcp:192.168.1.1:8080,downtime_ms=150,timeout_s=900,timeout_strategy=ignore,connections=4,tls_dir={}", tls_dir_path.display()))
                .unwrap();
        assert_eq!(
            data,
            VmSendMigrationData {
                destination_url: "tcp:192.168.1.1:8080".to_string(),
                local: false,
                downtime_ms: NonZeroU64::new(150).unwrap(),
                timeout_s: NonZeroU64::new(900).unwrap(),
                timeout_strategy: TimeoutStrategy::Ignore,
                connections: NonZeroU32::new(4).unwrap(),
                tls_dir: Some(tls_dir_path),
                memory_mode: MigrationMode::Precopy,
            }
        );

        // Postcopy happy path (TCP only, single connection).
        let data =
            VmSendMigrationData::parse("destination_url=tcp:192.168.1.1:8080,memory_mode=postcopy")
                .unwrap();
        assert_eq!(data.memory_mode, MigrationMode::Postcopy);
    }
}
