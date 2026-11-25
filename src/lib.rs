//! Offline estimate of orphaned EBS volume and snapshot cost.
//!
//! The library reads bytes you already have. It does not open a socket, it does
//! not link an AWS SDK, and it does not delete a volume or a snapshot.

mod bill;
mod error;
mod hidden;
mod json_scan;
mod money;
mod parse;
mod price;
mod report;
mod run;

pub use bill::{
    io2_iops_usd, kind_of, older_than, quote_snapshot, quote_volume, Bill, Quote, SnapshotBasis,
    SnapshotRow, VolumeKind, VolumeRow,
};
pub use error::Error;
pub use hidden::{escape_hidden, is_hidden};
pub use money::{
    days_from_civil, epoch_millis, format_dec, format_usd, gb_from_bytes, loses_digits,
    parse_utc_millis, Amount,
};
pub use parse::{
    parse_document, region_from_az, valid_image_id, valid_snapshot_id, valid_volume_id, Document,
    Image, Snapshot, SnapshotState, SnapshotTier, Volume,
};
pub use price::{load_prices_str, prices_builtin, NotApplied, Prices};
pub use report::{
    free_tier_line, io2_line, not_applied_line, render, render_markdown, throughput_line,
    NO_DELETE_LINE, SNAPSHOT_LINE,
};
pub use run::{bill_from_inputs, execute_paths, shown_path, NamedBytes, Options, Output};

/// Largest input accepted per file, in bytes (256 MiB).
pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
