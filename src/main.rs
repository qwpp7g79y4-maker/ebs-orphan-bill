use std::io::{stdout, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use rust_decimal::Decimal;

#[allow(unsafe_code)]
mod interrupt;
#[allow(unsafe_code)]
mod stdout_probe;

use ebs_wees_bill::{
    escape_hidden, execute_paths, is_hidden, load_prices_str, shown_path, Error, Options,
};

/// The crate version. A debug build made in a git checkout adds the short commit.
#[cfg(ebs_wees_bill_git_sha)]
const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("EBS_WEES_BILL_GIT_SHA"),
    ")"
);
#[cfg(not(ebs_wees_bill_git_sha))]
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Find loose EBS volumes and old snapshots without an AMI, and their monthly cost. Reads files only, no network.
#[derive(Parser)]
#[command(
    name = "ebs-wees-bill",
    bin_name = "ebs-wees-bill",
    version = VERSION,
    about = "Find loose EBS volumes and old snapshots without an AMI, and their monthly cost. Reads files only, no network.",
    after_help = "Exit codes: 0 ok, 1 a priced orphan costs more than --fail-usd, 2 input or usage error, or standard output that cannot be written."
)]
struct Cli {
    /// `describe-volumes`, `describe-snapshots`, or `describe-images` JSON. Files, gzip, directories, or `-` for standard input.
    #[arg(required = true, value_name = "FILE")]
    files: Vec<PathBuf>,
    /// Price file. The default is the built-in US East (N. Virginia) EBS list.
    #[arg(long, value_name = "FILE")]
    prices: Option<PathBuf>,
    /// Date ages are measured to, `YYYY-MM-DD`, midnight UTC. Without it, snapshots are not classified.
    #[arg(long, value_name = "YYYY-MM-DD")]
    as_of: Option<String>,
    /// A snapshot is old when its age is strictly greater than this many days.
    #[arg(long, value_name = "DAYS", default_value_t = 30)]
    older_than_days: u32,
    /// Exit 1 when a priced orphan's exact monthly USD is strictly greater than this.
    #[arg(long, value_name = "AMOUNT")]
    fail_usd: Option<String>,
    /// Print JSON instead of the text table.
    #[arg(long)]
    json: bool,
    /// Print a Markdown report (tables) instead of the text table.
    #[arg(long, conflicts_with = "json")]
    md: bool,
    /// Write the report to this file instead of standard output. The file is
    /// replaced whole or not at all; a symlink or a non-regular file is refused.
    #[arg(long, value_name = "FILE")]
    output: Option<PathBuf>,
    /// Hide the warnings; print only the result and errors.
    #[arg(long, conflicts_with = "verbose")]
    quiet: bool,
    /// Print progress on standard error: each input read, then one line of counts.
    #[arg(long, conflicts_with = "quiet")]
    verbose: bool,
}

fn main() -> ExitCode {
    interrupt::install();
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            if matches!(
                err.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                return if write_stdout(&err.render().to_string()) {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(2)
                };
            }
            let code = if err.use_stderr() { 2 } else { 0 };
            let args: Vec<String> = std::env::args_os()
                .skip(1)
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect();
            if code == 2 && args.iter().any(|arg| arg.chars().any(is_hidden)) {
                let _ = write!(std::io::stderr(), "{}", safe_usage_text(&args, &err));
            } else {
                let _ = err.print();
            }
            return ExitCode::from(code);
        }
    };
    if cli.verbose {
        for path in &cli.files {
            let _ = writeln!(std::io::stderr(), "reading {}", shown_path(path));
        }
    }
    match run(&cli) {
        Ok(output) => {
            if cli.verbose {
                let _ = writeln!(std::io::stderr(), "{}", output.context);
            }
            for warning in &output.warnings {
                if cli.json || warning.starts_with("price data is ") {
                    let _ = writeln!(std::io::stderr(), "warning: {warning}");
                }
            }
            if let Some(path) = &cli.output {
                if let Err(err) = write_file_atomic(path, output.stdout.as_bytes()) {
                    let _ = writeln!(
                        std::io::stderr(),
                        "ebs-wees-bill: cannot write {}: {err}",
                        shown_path(path)
                    );
                    return ExitCode::from(2);
                }
            } else if !write_stdout(&output.stdout) {
                return ExitCode::from(2);
            }
            if interrupt::pending() {
                let _ = write!(std::io::stderr(), "{}", interrupt::MESSAGE);
                return ExitCode::from(interrupt::EXIT_SIGINT);
            }
            if output.threshold_exceeded {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(err) => {
            let _ = writeln!(std::io::stderr(), "ebs-wees-bill: {err}");
            ExitCode::from(2)
        }
    }
}

/// Writes `bytes` to a new temporary file next to `path` and renames it over
/// `path`, so a reader sees the old file or the whole new one. A symlink or a
/// file that is not a regular file is refused; nothing is followed.
fn write_file_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::fs::OpenOptions;
    use std::path::Path;
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err("the target is a symlink, which is not followed".to_string())
        }
        Ok(meta) if !meta.is_file() => return Err("the target is not a regular file".to_string()),
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.to_string()),
    }
    let name = path
        .file_name()
        .ok_or_else(|| "the target has no file name".to_string())?;
    let dir = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let mut attempt = 0u32;
    let (mut file, temp) = loop {
        let mut temp_name = std::ffi::OsString::from(".");
        temp_name.push(name);
        temp_name.push(format!(".tmp-{}-{attempt}", std::process::id()));
        let temp = dir.join(temp_name);
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => break (file, temp),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists && attempt < 100 => {
                attempt += 1;
            }
            Err(err) => return Err(err.to_string()),
        }
    };
    interrupt::set_cleanup(Some(&temp));
    let result = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| {
            drop(file);
            std::fs::rename(&temp, path)
        });
    interrupt::set_cleanup(None);
    if let Err(err) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(err.to_string());
    }
    Ok(())
}

/// Writes to stdout; on a closed or failing stdout prints the error and returns false.
fn write_stdout(text: &str) -> bool {
    let mut out = stdout().lock();
    interrupt::set_writing(true);
    let written = stdout_is_open() && out.write_all(text.as_bytes()).is_ok() && out.flush().is_ok();
    interrupt::set_writing(false);
    if !written {
        let _ = writeln!(std::io::stderr(), "ebs-wees-bill: could not write output");
        return false;
    }
    true
}

/// False only when fd 1 was already closed at process start (`>&-`).
/// `1<>/dev/null` and `>/dev/null` stay open. The check is one `fcntl` call before
/// `main`; it does not use `/proc`.
fn stdout_is_open() -> bool {
    !stdout_probe::stdout_was_closed_at_start()
}

/// Clap echoes arguments in its messages, and it strips some control characters
/// from what it echoes, so the raw argument cannot be found again in the message.
/// The message is built from the arguments after every hidden character is a
/// `\u{..}` escape, so each line break in it is clap's own. When the escaped
/// arguments parse (clap refused the real ones, for example for a value that is
/// not UTF-8), the message of the real error is used; it echoes no argument.
fn safe_usage_text(args: &[String], real: &clap::Error) -> String {
    let escaped: Vec<String> = std::iter::once("ebs-wees-bill".to_string())
        .chain(args.iter().map(|arg| escape_hidden(arg)))
        .collect();
    let text = match Cli::try_parse_from(escaped) {
        Err(err) => err.render().to_string(),
        Ok(_) => real.render().to_string(),
    };
    text.chars()
        .flat_map(|c| {
            if c != '\n' && is_hidden(c) {
                c.escape_unicode().collect::<Vec<char>>()
            } else {
                vec![c]
            }
        })
        .collect()
}

fn run(cli: &Cli) -> Result<ebs_wees_bill::Output, Error> {
    let prices = if let Some(path) = &cli.prices {
        let text = read_price_file(path)?;
        load_prices_str(&text)
            .map_err(|err| Error::new(format!("price file {}: {err}", shown_path(path))))?
    } else {
        ebs_wees_bill::prices_builtin()
    };
    let fail_usd = match &cli.fail_usd {
        Some(text) => Some(parse_fail(text)?),
        None => None,
    };
    let options = Options {
        prices,
        as_of: cli.as_of.clone(),
        older_than_days: cli.older_than_days,
        fail_usd,
        json: cli.json,
        markdown: cli.md,
        quiet: cli.quiet,
        today_ms: today_ms(),
    };
    execute_paths(&cli.files, &options)
}

/// Midnight UTC of today, or `None` when the clock is before 1970.
fn today_ms() -> Option<i64> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    i64::try_from(secs / 86_400 * 86_400 * 1000).ok()
}

fn read_price_file(path: &std::path::Path) -> Result<String, Error> {
    let meta = std::fs::metadata(path)
        .map_err(|err| Error::new(format!("{}: {err}", shown_path(path))))?;
    if !meta.is_file() {
        return Err(Error::new(format!("{}: not a file", shown_path(path))));
    }
    let len = meta.len();
    if len > ebs_wees_bill::MAX_FILE_BYTES {
        return Err(Error::new(format!(
            "{}: file is larger than 268435456 bytes",
            shown_path(path)
        )));
    }
    let file = std::fs::File::open(path)
        .map_err(|err| Error::new(format!("{}: {err}", shown_path(path))))?;
    let mut text = String::new();
    // Stop at the size metadata already reported.
    file.take(len)
        .read_to_string(&mut text)
        .map_err(|err| Error::new(format!("{}: {err}", shown_path(path))))?;
    Ok(text)
}

fn parse_fail(text: &str) -> Result<Decimal, Error> {
    let bytes = text.as_bytes();
    if text.is_empty() || text.len() > 40 {
        return Err(Error::new("invalid --fail-usd"));
    }
    let mut dot = false;
    for (i, byte) in bytes.iter().enumerate() {
        match byte {
            b'0'..=b'9' => {}
            b'.' if !dot && i > 0 && i + 1 < bytes.len() => dot = true,
            _ => return Err(Error::new("invalid --fail-usd")),
        }
    }
    if text
        .split_once('.')
        .is_some_and(|(_, frac)| frac.len() > 28)
    {
        return Err(Error::new("--fail-usd does not fit: more than 28 decimals"));
    }
    let value: Decimal = text.parse().map_err(|_| Error::new("invalid --fail-usd"))?;
    if ebs_wees_bill::loses_digits(text) {
        return Err(Error::new(
            "--fail-usd does not fit: more digits than a decimal holds exactly",
        ));
    }
    Ok(value)
}
