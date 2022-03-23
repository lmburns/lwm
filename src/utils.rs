//! Various helper-utilities

use crate::{cli::Opts, config::Config};
use anyhow::{Context, Result};
use clap::crate_name;
use flexi_logger::{
    opt_format,
    style,
    AdaptiveFormat,
    Age,
    Cleanup,
    Criterion,
    DeferredNow,
    Duplicate,
    FileSpec,
    FlexiLoggerError,
    Level,
    Logger,
    LoggerHandle,
    Naming,
    Record,
    WriteMode,
};
use log::LevelFilter;
use serde::{de, Deserialize};
use shellexpand::LookupError;
use std::{
    borrow::Cow,
    env,
    hash::{BuildHasherDefault, Hasher},
    io::{self, Write},
    panic,
    path::PathBuf,
    thread,
};
use which::which;

/// Used as a custom inner state/hasher for any `Hash` item in the [`std`]
#[derive(Default)]
pub(crate) struct IdHasher {
    /// Current state of the hasher
    state: u64,
}

impl Hasher for IdHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state = self.state.rotate_left(8) + u64::from(byte);
        }
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.state
    }
}

/// Type alias to build a `Hash` using [`IdHasher`]
pub(crate) type BuildIdHasher = BuildHasherDefault<IdHasher>;

/// Shorter way of testing if the user wants color for the output of `--help`
pub(crate) fn wants_color() -> bool {
    env::var_os("NO_COLOR").is_none()
}

// TODO: Perhaps use a `SyslogWriter`

/// Initializes logging for this crate
pub(crate) fn initialize_logging(config: &Config, args: &Opts) -> Result<PathBuf> {
    /// Customize the format of the log (colored)
    fn colored_format(
        w: &mut dyn Write,
        now: &mut DeferredNow,
        record: &Record,
    ) -> Result<(), io::Error> {
        let level = record.level();
        // style(level, now.now().format("%d %H:%M:%S")),
        write!(
            w,
            "{:<5} [{}:{}]: {}",
            style(level, level),
            style(Level::Trace, record.file().unwrap_or("<unnamed>")),
            record.line().unwrap_or(0),
            &record.args() // style(level, &record.args())
        )
    }

    /// Customize the format of the log (uncolored)
    fn uncolored_format(
        w: &mut dyn Write,
        now: &mut DeferredNow,
        record: &Record,
    ) -> Result<(), io::Error> {
        // Strip the ansi sequences that I have put in log messages using the `colored`
        // crate when writing to a file. Also use a date
        write!(
            w,
            "[{:>}] {:<5} [{}:{}]: {}",
            now.now().format("%Y-%m-%d %H:%M:%S"),
            record.level(),
            record.file().unwrap_or("<unnamed>"),
            record.line().unwrap_or(0),
            String::from_utf8(strip_ansi_escapes::strip(
                &record.args().to_string().as_bytes()
            )?)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        )
    }

    // This provides much better backtraces, in a Python manner. This makes it
    // easier to see exactly where errors have occured and is useful with this crate
    // because of the communication with the X-Server
    if cfg!(debug_assertions) {
        better_panic::install();
        panic::set_hook(Box::new(|panic_info| {
            better_panic::Settings::auto().create_panic_handler()(panic_info);
        }));
    }

    let log_dir = config.global.log_dir.as_ref().map_or_else(
        || env::temp_dir().join(crate_name!()),
        |dir| {
            PathBuf::from(
                shellexpand::full(&dir.display().to_string())
                    .unwrap_or_else(|_| {
                        Cow::from(
                            LookupError {
                                var_name: "Unkown Environment Variable".into(),
                                cause:    env::VarError::NotPresent,
                            }
                            .to_string(),
                        )
                    })
                    .to_string(),
            )
        },
    );

    // .create_symlink()
    // .format(colored_format)
    let mut logger = Logger::try_with_str(env::var("LXHKD_LOG").unwrap_or_else(
        |_| match args.verbose {
            1 => String::from("debug"),
            2 => String::from("trace"),
            _ => String::from("info"),
        },
    ))?
    .write_mode(WriteMode::BufferAndFlush)
    .adaptive_format_for_stderr(AdaptiveFormat::Custom(uncolored_format, colored_format))
    .set_palette(String::from("9;11;14;5;13"));

    if config.global.log_to_file {
        logger = logger
            .duplicate_to_stderr(Duplicate::All)
            .rotate(
                Criterion::AgeOrSize(Age::Day, 50_000_000),
                Naming::Numbers,
                Cleanup::KeepLogFiles(2),
            )
            .log_to_file(
                FileSpec::default()
                    .basename(crate_name!())
                    .directory(&log_dir),
            )
            .format_for_files(uncolored_format);
    }

    logger.start();

    Ok(log_dir)
}

/// [`Deserialize`] something that has a shell variable
#[allow(single_use_lifetimes)]
pub(crate) fn deserialize_shellexpand<'de, D>(d: D) -> Result<Option<PathBuf>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let value = PathBuf::deserialize(d)?;

    let value = PathBuf::from(
        shellexpand::full(&value.to_string_lossy())
            .map_err(|e| {
                de::Error::invalid_value(
                    de::Unexpected::Str(value.to_string_lossy().as_ref()),
                    &e.to_string().as_str(),
                )
            })?
            .to_string(),
    );

    Ok(Some(value))
}

/// [`Deserialize`] something that has a shell variable into an absolute path
#[allow(single_use_lifetimes)]
pub(crate) fn deserialize_absolute_path<'de, D>(d: D) -> Result<Option<PathBuf>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let value = PathBuf::deserialize(d)?;

    let value = PathBuf::from(
        shellexpand::full(&value.to_string_lossy())
            .map_err(|e| {
                de::Error::invalid_value(
                    de::Unexpected::Str(value.to_string_lossy().as_ref()),
                    &e.to_string().as_str(),
                )
            })?
            .to_string(),
    );

    let canonicalize = |p: &PathBuf| -> Result<PathBuf, D::Error> {
        p.canonicalize()
            .map_err(|e| de::Error::custom(format!("failed to canonicalize path: {}", p.display())))
    };

    // Maybe this could be cleaned up?

    // Canonicalize the path
    if let Ok(value) = canonicalize(&value) {
        // Return an error if the path isn't absolute
        if !value.is_absolute() {
            // Maybe it was a binary name given
            // `which` should return the absolute value
            if let Ok(ret) = which(&value) {
                return Ok(Some(canonicalize(&ret)?));
            }

            return Err(de::Error::invalid_value(
                de::Unexpected::Str(value.to_string_lossy().as_ref()),
                &"path must be absolute XX1",
            ));
        }

        Ok(Some(value))
    } else {
        if let Ok(ret) = which(&value) {
            return Ok(Some(canonicalize(&ret)?));
        }

        Err(de::Error::invalid_value(
            de::Unexpected::Str(value.to_string_lossy().as_ref()),
            &"path must be absolute XX2",
        ))
    }
}

// ]]] === Project Dirs ===
