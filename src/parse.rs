use std::io::Read;

use flate2::bufread::GzDecoder;
use serde_json::{Map, Value};

use crate::error::Error;
use crate::json_scan::{top_level_keys, KeyFault, MAX_JSON_ITEMS};
use crate::money::{parse_utc_millis, trim_ascii_ws};
use crate::MAX_FILE_BYTES;

/// One EBS volume. Attachment bodies, keys, and owner ids are not kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    pub id: String,
    pub volume_type: String,
    pub size_gb: u64,
    pub iops: Option<u64>,
    pub throughput_mib: Option<u64>,
    pub loose: bool,
    pub state_in_use: bool,
    /// A State where the volume is not a settled orphan: creating, deleting, deleted, or error.
    pub transient_state: Option<&'static str>,
    pub region: Option<String>,
    pub az_unparsed: bool,
}

/// Snapshot storage tier after a case-insensitive match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotTier {
    Standard,
    Archive,
    Other,
}

/// Snapshot state. A missing state is [`SnapshotState::Completed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotState {
    Completed,
    Pending,
    Error,
    Other,
}

/// One snapshot. Description and owner id are not kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub id: String,
    pub start_ms: Option<i64>,
    pub volume_size_gb: Option<u64>,
    pub full_bytes: Option<u64>,
    pub tier: SnapshotTier,
    pub state: SnapshotState,
}

/// One image. Only the id and the snapshot ids it references are kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub id: String,
    pub snapshot_ids: Vec<String>,
}

/// One input document. At most one of the three lists is filled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub volumes: Vec<Volume>,
    pub snapshots: Vec<Snapshot>,
    pub images: Vec<Image>,
    pub warnings: Vec<String>,
    pub saw_volumes: bool,
    pub saw_snapshots: bool,
    pub saw_images: bool,
}

#[must_use]
pub fn valid_volume_id(text: &str) -> bool {
    valid_id(text, "vol-")
}

#[must_use]
pub fn valid_snapshot_id(text: &str) -> bool {
    valid_id(text, "snap-")
}

#[must_use]
pub fn valid_image_id(text: &str) -> bool {
    valid_id(text, "ami-")
}

fn valid_id(text: &str, prefix: &str) -> bool {
    let Some(rest) = text.strip_prefix(prefix) else {
        return false;
    };
    (8..=17).contains(&rest.len())
        && rest
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Availability zone `us-east-1a` becomes region `us-east-1`.
/// A value that is not a lowercase zone or region returns `None`.
#[must_use]
pub fn region_from_az(az: &str) -> Option<String> {
    if !(8..=32).contains(&az.len()) {
        return None;
    }
    if !az
        .bytes()
        .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-'))
    {
        return None;
    }
    let parts: Vec<&str> = az.split('-').collect();
    if parts.len() < 3 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    if parts[0].len() < 2 || !parts[0].bytes().all(|byte| byte.is_ascii_lowercase()) {
        return None;
    }
    let last = parts[parts.len() - 1].as_bytes();
    let tail = last.last().copied()?;
    if tail.is_ascii_lowercase() {
        if last.len() < 2 || !last[last.len() - 2].is_ascii_digit() {
            return None;
        }
        if !last[..last.len() - 1]
            .iter()
            .all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        return Some(az[..az.len() - 1].to_string());
    }
    if last.iter().all(|byte| byte.is_ascii_digit()) {
        Some(az.to_string())
    } else {
        None
    }
}

pub fn parse_document(name: &str, bytes: &[u8], strict: bool) -> Result<Document, Error> {
    let raw = decompress(name, bytes)?;
    let text = match std::str::from_utf8(&raw) {
        Ok(text) => strip_bom(text),
        Err(_) if !strict => {
            return Ok(empty_skip(format!("skipped non-utf8 file {name}")));
        }
        Err(_) => return Err(Error::new(format!("{name}: not JSON"))),
    };
    if text
        .bytes()
        .all(|byte| byte.is_ascii_whitespace() || byte == 0x0b)
    {
        if strict {
            return Err(Error::new(format!("{name}: not JSON")));
        }
        return Ok(empty_skip(format!("skipped empty file {name}")));
    }
    match top_level_keys(text) {
        Ok(_) => {}
        Err(KeyFault::Duplicate) => return Err(Error::new(format!("{name}: duplicate key"))),
        Err(KeyFault::Bad) => return Err(Error::new(format!("{name}: not JSON"))),
        Err(KeyFault::TooMany) => {
            return Err(Error::new(format!(
                "{name}: too many JSON values (at most {MAX_JSON_ITEMS})"
            )))
        }
    }
    let value: Value =
        serde_json::from_str(text).map_err(|_| Error::new(format!("{name}: not JSON")))?;
    interpret(name, &value)
}

fn empty_skip(warning: String) -> Document {
    Document {
        volumes: Vec::new(),
        snapshots: Vec::new(),
        images: Vec::new(),
        warnings: vec![warning],
        saw_volumes: false,
        saw_snapshots: false,
        saw_images: false,
    }
}

fn decompress(name: &str, bytes: &[u8]) -> Result<Vec<u8>, Error> {
    if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        let mut rest: &[u8] = bytes;
        let mut out = Vec::new();
        GzDecoder::new(&mut rest)
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut out)
            .map_err(|_| Error::new(format!("{name}: could not read gzip")))?;
        if !rest.is_empty() && out.len() as u64 <= MAX_FILE_BYTES {
            return Err(Error::new(format!(
                "{name}: unexpected bytes after the first gzip member"
            )));
        }
        if out.len() as u64 > MAX_FILE_BYTES {
            return Err(Error::new(format!(
                "{name}: file is larger than 268435456 bytes"
            )));
        }
        Ok(out)
    } else {
        Ok(bytes.to_vec())
    }
}

fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

fn interpret(file: &str, value: &Value) -> Result<Document, Error> {
    match value {
        Value::Array(items) => from_array(file, items),
        Value::Object(map) => from_object(file, map),
        _ => Err(Error::new(format!(
            "{file}: expected volumes, snapshots, or images"
        ))),
    }
}

fn from_object(file: &str, map: &Map<String, Value>) -> Result<Document, Error> {
    let volumes = map.contains_key("Volumes");
    let snapshots = map.contains_key("Snapshots");
    let images = map.contains_key("Images");
    let kinds = usize::from(volumes) + usize::from(snapshots) + usize::from(images);
    if kinds > 1 {
        return Err(Error::new(format!("{file}: mixed document")));
    }
    let mut warnings = Vec::new();
    note_next_token(file, map, &mut warnings)?;
    if volumes {
        let items = array_field(file, map, "Volumes")?;
        let parsed = volumes_from(file, items)?;
        return Ok(Document {
            volumes: parsed,
            snapshots: Vec::new(),
            images: Vec::new(),
            warnings,
            saw_volumes: true,
            saw_snapshots: false,
            saw_images: false,
        });
    }
    if snapshots {
        let items = array_field(file, map, "Snapshots")?;
        let parsed = snapshots_from(file, items)?;
        return Ok(Document {
            volumes: Vec::new(),
            snapshots: parsed,
            images: Vec::new(),
            warnings,
            saw_volumes: false,
            saw_snapshots: true,
            saw_images: false,
        });
    }
    if images {
        let items = array_field(file, map, "Images")?;
        let parsed = images_from(file, items)?;
        return Ok(Document {
            volumes: Vec::new(),
            snapshots: Vec::new(),
            images: parsed,
            warnings,
            saw_volumes: false,
            saw_snapshots: false,
            saw_images: true,
        });
    }
    match resource_kind(map) {
        Some(ResourceKind::Volume) => {
            // The same object inside an array is a mixed document, so it is one here too.
            if map.contains_key("ImageId") {
                return Err(Error::new(format!("{file}: mixed document")));
            }
            let volume = one_volume(file, map)?;
            Ok(Document {
                volumes: vec![volume],
                snapshots: Vec::new(),
                images: Vec::new(),
                warnings,
                saw_volumes: true,
                saw_snapshots: false,
                saw_images: false,
            })
        }
        Some(ResourceKind::Snapshot) => {
            let snapshot = one_snapshot(file, map)?;
            Ok(Document {
                volumes: Vec::new(),
                snapshots: vec![snapshot],
                images: Vec::new(),
                warnings,
                saw_volumes: false,
                saw_snapshots: true,
                saw_images: false,
            })
        }
        Some(ResourceKind::Image) => {
            let image = one_image(file, map)?;
            Ok(Document {
                volumes: Vec::new(),
                snapshots: Vec::new(),
                images: vec![image],
                warnings,
                saw_volumes: false,
                saw_snapshots: false,
                saw_images: true,
            })
        }
        None => Err(Error::new(format!(
            "{file}: expected volumes, snapshots, or images"
        ))),
    }
}

fn from_array(file: &str, items: &[Value]) -> Result<Document, Error> {
    let Some(first) = items.first() else {
        return Err(Error::new(format!(
            "{file}: expected volumes, snapshots, or images"
        )));
    };
    let Some(obj) = first.as_object() else {
        return Err(Error::new(format!(
            "{file}: expected volumes, snapshots, or images"
        )));
    };
    match resource_kind(obj) {
        Some(ResourceKind::Volume) => Ok(Document {
            volumes: volumes_from(file, items)?,
            snapshots: Vec::new(),
            images: Vec::new(),
            warnings: Vec::new(),
            saw_volumes: true,
            saw_snapshots: false,
            saw_images: false,
        }),
        Some(ResourceKind::Snapshot) => Ok(Document {
            volumes: Vec::new(),
            snapshots: snapshots_from(file, items)?,
            images: Vec::new(),
            warnings: Vec::new(),
            saw_volumes: false,
            saw_snapshots: true,
            saw_images: false,
        }),
        Some(ResourceKind::Image) => Ok(Document {
            volumes: Vec::new(),
            snapshots: Vec::new(),
            images: images_from(file, items)?,
            warnings: Vec::new(),
            saw_volumes: false,
            saw_snapshots: false,
            saw_images: true,
        }),
        None => Err(Error::new(format!(
            "{file}: expected volumes, snapshots, or images"
        ))),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResourceKind {
    Volume,
    Snapshot,
    Image,
}

/// `describe-volumes` carries `SnapshotId` (often `""`).
/// `describe-snapshots` carries `VolumeId`.
/// A volume shape wins, so a source snapshot id stays a volume.
fn resource_kind(obj: &Map<String, Value>) -> Option<ResourceKind> {
    let volume_shape = obj.contains_key("Size")
        || obj.contains_key("VolumeType")
        || obj.contains_key("Attachments");
    if volume_shape && (obj.contains_key("VolumeId") || obj.contains_key("SnapshotId")) {
        return Some(ResourceKind::Volume);
    }
    if snapshot_identity(obj) {
        return Some(ResourceKind::Snapshot);
    }
    if obj.contains_key("VolumeId") {
        return Some(ResourceKind::Volume);
    }
    if obj.contains_key("ImageId") {
        return Some(ResourceKind::Image);
    }
    None
}

/// A non-empty `SnapshotId` is a snapshot. `""` is the volume field from describe-volumes.
fn snapshot_identity(obj: &Map<String, Value>) -> bool {
    match obj.get("SnapshotId") {
        Some(Value::String(text)) => !text.is_empty(),
        Some(_) => true,
        None => false,
    }
}

fn array_field<'a>(
    file: &str,
    map: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a Vec<Value>, Error> {
    match map.get(key) {
        Some(Value::Array(items)) => Ok(items),
        _ => Err(Error::new(format!("{file}: bad {key}"))),
    }
}

fn note_next_token(
    file: &str,
    map: &Map<String, Value>,
    warnings: &mut Vec<String>,
) -> Result<(), Error> {
    let mut warned = false;
    for key in ["nextToken", "NextToken"] {
        let Some(token) = map.get(key) else {
            continue;
        };
        match token {
            Value::Null => {}
            Value::String(text) if text.is_empty() => {}
            Value::String(_) => {
                if !warned {
                    warnings.push(format!(
                        "{file}: page may be partial; nextToken was not followed"
                    ));
                    warned = true;
                }
            }
            _ => return Err(Error::new(format!("{file}: bad nextToken"))),
        }
    }
    Ok(())
}

fn volumes_from(file: &str, items: &[Value]) -> Result<Vec<Volume>, Error> {
    let mut volumes = Vec::with_capacity(items.len());
    for item in items {
        let Some(obj) = item.as_object() else {
            return Err(Error::new(format!(
                "{file}: expected volumes, snapshots, or images"
            )));
        };
        if matches!(
            resource_kind(obj),
            Some(ResourceKind::Snapshot) | Some(ResourceKind::Image)
        ) || obj.contains_key("ImageId")
        {
            return Err(Error::new(format!("{file}: mixed document")));
        }
        volumes.push(one_volume(file, obj)?);
    }
    Ok(volumes)
}

fn snapshots_from(file: &str, items: &[Value]) -> Result<Vec<Snapshot>, Error> {
    let mut snapshots = Vec::with_capacity(items.len());
    for item in items {
        let Some(obj) = item.as_object() else {
            return Err(Error::new(format!(
                "{file}: expected volumes, snapshots, or images"
            )));
        };
        if matches!(
            resource_kind(obj),
            Some(ResourceKind::Volume) | Some(ResourceKind::Image)
        ) || obj.contains_key("ImageId")
        {
            return Err(Error::new(format!("{file}: mixed document")));
        }
        snapshots.push(one_snapshot(file, obj)?);
    }
    Ok(snapshots)
}

fn images_from(file: &str, items: &[Value]) -> Result<Vec<Image>, Error> {
    let mut images = Vec::with_capacity(items.len());
    for item in items {
        let Some(obj) = item.as_object() else {
            return Err(Error::new(format!(
                "{file}: expected volumes, snapshots, or images"
            )));
        };
        if obj.contains_key("VolumeId") || obj.contains_key("SnapshotId") {
            return Err(Error::new(format!("{file}: mixed document")));
        }
        images.push(one_image(file, obj)?);
    }
    Ok(images)
}

fn one_volume(file: &str, obj: &Map<String, Value>) -> Result<Volume, Error> {
    let id = req_id(file, obj, "VolumeId", valid_volume_id)?;
    let volume_type = match obj.get("VolumeType") {
        Some(Value::String(text)) => {
            let text = trim_ascii_ws(text).to_ascii_lowercase();
            if text.is_empty() || text.len() > 64 {
                return Err(Error::new(format!("{file}: {id}: bad VolumeType")));
            }
            text
        }
        _ => return Err(Error::new(format!("{file}: {id}: bad VolumeType"))),
    };
    let size_gb = match obj.get("Size") {
        Some(value) => {
            json_u64(value).map_err(|()| Error::new(format!("{file}: {id}: bad Size")))?
        }
        None => return Err(Error::new(format!("{file}: {id}: missing Size"))),
    };
    let iops = optional_u64(file, &id, obj, "Iops")?;
    let throughput_mib = optional_u64(file, &id, obj, "Throughput")?;
    let loose = attachments_empty(file, &id, obj)?;
    let state_in_use = match obj.get("State") {
        None | Some(Value::Null) => false,
        Some(Value::String(text)) => text == "in-use",
        Some(_) => return Err(Error::new(format!("{file}: {id}: bad State"))),
    };
    let transient_state = match obj.get("State") {
        Some(Value::String(text)) => ["creating", "deleting", "deleted", "error"]
            .into_iter()
            .find(|name| text == name),
        _ => None,
    };
    let (region, az_unparsed) = match obj.get("AvailabilityZone") {
        None | Some(Value::Null) => (None, false),
        Some(Value::String(text)) => match region_from_az(text) {
            Some(region) => (Some(region), false),
            None => (None, true),
        },
        Some(_) => return Err(Error::new(format!("{file}: {id}: bad AvailabilityZone"))),
    };
    Ok(Volume {
        id,
        volume_type,
        size_gb,
        iops,
        throughput_mib,
        loose,
        state_in_use,
        transient_state,
        region,
        az_unparsed,
    })
}

fn attachments_empty(file: &str, id: &str, obj: &Map<String, Value>) -> Result<bool, Error> {
    match obj.get("Attachments") {
        None | Some(Value::Null) => Ok(true),
        Some(Value::Array(items)) => {
            for item in items {
                if !item.is_object() {
                    return Err(Error::new(format!("{file}: {id}: bad Attachments")));
                }
            }
            Ok(items.is_empty())
        }
        Some(_) => Err(Error::new(format!("{file}: {id}: bad Attachments"))),
    }
}

fn one_snapshot(file: &str, obj: &Map<String, Value>) -> Result<Snapshot, Error> {
    let id = req_id(file, obj, "SnapshotId", valid_snapshot_id)?;
    let start_ms = match obj.get("StartTime") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(
            parse_utc_millis(text)
                .ok_or_else(|| Error::new(format!("{file}: {id}: bad StartTime")))?,
        ),
        Some(_) => return Err(Error::new(format!("{file}: {id}: bad StartTime"))),
    };
    let volume_size_gb = optional_u64(file, &id, obj, "VolumeSize")?;
    let full_bytes = optional_u64(file, &id, obj, "FullSnapshotSizeInBytes")?;
    if volume_size_gb.is_none() && full_bytes.is_none() {
        return Err(Error::new(format!("{file}: {id}: missing size")));
    }
    let tier = match obj.get("StorageTier") {
        None | Some(Value::Null) => SnapshotTier::Standard,
        Some(Value::String(text)) => match trim_ascii_ws(text).to_ascii_lowercase().as_str() {
            "standard" => SnapshotTier::Standard,
            "archive" => SnapshotTier::Archive,
            _ => SnapshotTier::Other,
        },
        Some(_) => return Err(Error::new(format!("{file}: {id}: bad StorageTier"))),
    };
    let state = match obj.get("State") {
        None | Some(Value::Null) => SnapshotState::Completed,
        Some(Value::String(text)) => match trim_ascii_ws(text).to_ascii_lowercase().as_str() {
            "completed" => SnapshotState::Completed,
            "pending" => SnapshotState::Pending,
            "error" => SnapshotState::Error,
            _ => SnapshotState::Other,
        },
        Some(_) => return Err(Error::new(format!("{file}: {id}: bad State"))),
    };
    Ok(Snapshot {
        id,
        start_ms,
        volume_size_gb,
        full_bytes,
        tier,
        state,
    })
}

fn one_image(file: &str, obj: &Map<String, Value>) -> Result<Image, Error> {
    let id = req_id(file, obj, "ImageId", valid_image_id)?;
    let mut snapshot_ids = Vec::new();
    if let Some(value) = obj.get("BlockDeviceMappings") {
        let Some(items) = value.as_array() else {
            return Err(Error::new(format!("{file}: {id}: bad BlockDeviceMappings")));
        };
        for item in items {
            let Some(mapping) = item.as_object() else {
                return Err(Error::new(format!("{file}: {id}: bad BlockDeviceMappings")));
            };
            let Some(ebs) = mapping.get("Ebs") else {
                continue;
            };
            let Some(ebs) = ebs.as_object() else {
                return Err(Error::new(format!("{file}: {id}: bad BlockDeviceMappings")));
            };
            let Some(snap) = ebs.get("SnapshotId") else {
                continue;
            };
            let Some(text) = snap.as_str() else {
                return Err(Error::new(format!("{file}: {id}: bad SnapshotId")));
            };
            if !valid_snapshot_id(text) {
                return Err(Error::new(format!("{file}: {id}: bad SnapshotId")));
            }
            snapshot_ids.push(text.to_string());
        }
    }
    Ok(Image { id, snapshot_ids })
}

fn req_id(
    file: &str,
    obj: &Map<String, Value>,
    key: &str,
    valid: fn(&str) -> bool,
) -> Result<String, Error> {
    match obj.get(key) {
        Some(Value::String(text)) if valid(text) => Ok(text.clone()),
        Some(_) => Err(Error::new(format!("{file}: bad {key}"))),
        None => Err(Error::new(format!("{file}: missing {key}"))),
    }
}

fn optional_u64(
    file: &str,
    id: &str,
    obj: &Map<String, Value>,
    key: &str,
) -> Result<Option<u64>, Error> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => json_u64(value)
            .map(Some)
            .map_err(|()| Error::new(format!("{file}: {id}: bad {key}"))),
    }
}

fn json_u64(value: &Value) -> Result<u64, ()> {
    let text = match value {
        Value::Number(number) => number.to_string(),
        Value::String(text) => trim_ascii_ws(text).to_string(),
        _ => return Err(()),
    };
    let whole = match text.split_once('.') {
        Some((head, frac)) if !frac.is_empty() && frac.bytes().all(|byte| byte == b'0') => head,
        Some(_) => return Err(()),
        None => text.as_str(),
    };
    if whole.is_empty() || whole.len() > 20 || !whole.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(());
    }
    whole.parse().map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::{
        parse_document, region_from_az, valid_image_id, valid_snapshot_id, valid_volume_id,
        SnapshotState, SnapshotTier,
    };

    #[test]
    fn u64_string_trims_only_ascii_whitespace() {
        let ok =
            doc("{\"VolumeId\":\"vol-01234567\",\"Size\":\" \\t80\\r\\n\",\"VolumeType\":\"gp2\"}");
        assert_eq!(ok.volumes[0].size_gb, 80);
        for ws in ["\\u00a0", "\\u2028", "\\u3000"] {
            for text in [format!("{ws}80"), format!("80{ws}")] {
                let json = format!(
                    "{{\"VolumeId\":\"vol-01234567\",\"Size\":\"{text}\",\"VolumeType\":\"gp2\"}}"
                );
                let err = parse_document("f.json", json.as_bytes(), true).unwrap_err();
                assert!(err.message().contains("bad Size"), "{}", err.message());
            }
        }
    }

    #[test]
    fn volume_type_trims_only_ascii_whitespace() {
        let ok = doc("{\"VolumeId\":\"vol-01234567\",\"Size\":8,\"VolumeType\":\" \\tGP2\\r\\n\"}");
        assert_eq!(ok.volumes[0].volume_type, "gp2");
        for ws in ["\\u00a0", "\\u2028", "\\u3000"] {
            for text in [format!("{ws}gp2"), format!("gp2{ws}")] {
                let json = format!(
                    "{{\"VolumeId\":\"vol-01234567\",\"Size\":8,\"VolumeType\":\"{text}\"}}"
                );
                let doc = parse_document("f.json", json.as_bytes(), true).unwrap();
                assert_ne!(doc.volumes[0].volume_type, "gp2", "{text}");
            }
        }
    }

    #[test]
    fn storage_tier_and_state_trim_only_ascii_whitespace() {
        let snap = |field: &str, value: &str| {
            doc(&format!(
                "{{\"SnapshotId\":\"snap-0123456789abcdef0\",\"VolumeSize\":2,\"{field}\":\"{value}\"}}"
            ))
            .snapshots
            .remove(0)
        };
        assert_eq!(
            snap("StorageTier", " \\tARCHIVE\\r\\n").tier,
            SnapshotTier::Archive
        );
        assert_eq!(
            snap("State", " \\tPENDING\\r\\n").state,
            SnapshotState::Pending
        );
        for ws in ["\\u00a0", "\\u2028", "\\u3000"] {
            for (pre, post) in [(ws, ""), ("", ws)] {
                let tier = snap("StorageTier", &format!("{pre}archive{post}")).tier;
                assert_eq!(tier, SnapshotTier::Other, "{ws}");
                let state = snap("State", &format!("{pre}pending{post}")).state;
                assert_eq!(state, SnapshotState::Other, "{ws}");
            }
        }
    }

    fn doc(json: &str) -> super::Document {
        parse_document("f.json", json.as_bytes(), true).unwrap()
    }

    #[test]
    fn unicode_whitespace_only_file_is_not_empty() {
        for pad in ["\u{a0}", "\u{2028}", "\u{3000}"] {
            for strict in [true, false] {
                let err = parse_document("f.json", pad.as_bytes(), strict).unwrap_err();
                assert!(err.to_string().contains("not JSON"), "{pad:?}");
            }
        }
        assert!(parse_document("f.json", b" \t\r\n", false)
            .unwrap()
            .warnings[0]
            .contains("skipped empty"));
    }

    #[test]
    fn ids_and_regions() {
        assert!(valid_volume_id("vol-01234567"));
        assert!(valid_volume_id("vol-11111111111111111"));
        assert!(!valid_volume_id("vol-0123456"));
        assert!(!valid_volume_id("vol-111111111111111111"));
        assert!(!valid_volume_id("vol-ABCDEF1234567890"));
        assert!(valid_snapshot_id("snap-11111111111111111"));
        assert!(valid_image_id("ami-11111111111111111"));
        assert!(!valid_image_id("snap-11111111111111111"));
        assert_eq!(region_from_az("us-east-1a").as_deref(), Some("us-east-1"));
        assert_eq!(region_from_az("eu-west-1b").as_deref(), Some("eu-west-1"));
        assert_eq!(
            region_from_az("ap-southeast-2a").as_deref(),
            Some("ap-southeast-2")
        );
        assert_eq!(region_from_az("us-east-1").as_deref(), Some("us-east-1"));
        assert!(region_from_az("US-EAST-1A").is_none());
        assert!(region_from_az("not a zone SECRET").is_none());
        assert!(region_from_az("us-east-1aa").is_none());
    }

    #[test]
    fn loose_attached_and_single_objects() {
        let parsed = doc(r#"{"Volumes":[
                {"VolumeId":"vol-11111111111111111","Size":100,"VolumeType":"gp3","Iops":4000,"Throughput":200,"Attachments":[],"State":"available","AvailabilityZone":"us-east-1a","KmsKeyId":"SECRETKEY","OwnerId":"123456789012"},
                {"VolumeId":"vol-33333333333333333","Size":"80","VolumeType":"io1","Attachments":[{"InstanceId":"i-11111111111111111","State":"detaching"}]}
            ]}"#);
        assert!(parsed.warnings.is_empty());
        assert!(parsed.volumes[0].loose);
        assert_eq!(parsed.volumes[0].region.as_deref(), Some("us-east-1"));
        assert!(!parsed.volumes[1].loose);
        assert_eq!(parsed.volumes[1].size_gb, 80);
        let one = doc(r#"{"VolumeId":"vol-01234567","Size":1,"VolumeType":"GP2"}"#);
        assert_eq!(one.volumes[0].volume_type, "gp2");
        assert!(one.volumes[0].loose);
        assert!(one.saw_volumes);
    }

    #[test]
    fn in_use_without_attachments_and_bad_id_is_hidden() {
        let parsed = doc(
            r#"{"VolumeId":"vol-aaaaaaaaaaaaaaaaa","Size":1,"VolumeType":"gp2","State":"in-use"}"#,
        );
        assert!(parsed.volumes[0].loose);
        assert!(parsed.volumes[0].state_in_use);
        let err = parse_document(
            "f.json",
            br#"{"VolumeId":"SECRETKEY","Size":1,"VolumeType":"gp2"}"#,
            true,
        )
        .unwrap_err();
        assert_eq!(err.message(), "f.json: bad VolumeId");
        assert!(!err.message().contains("SECRETKEY"));
    }

    #[test]
    fn snapshots_images_and_tiers() {
        let snaps = doc(r#"{"Snapshots":[
                {"SnapshotId":"snap-11111111111111111","VolumeSize":10,"StartTime":"2026-10-09T00:00:00.000Z","Description":"Created by CreateImage SECRETKEY"},
                {"SnapshotId":"snap-22222222222222222","FullSnapshotSizeInBytes":1073741824,"StorageTier":"archive","State":"pending"},
                {"SnapshotId":"snap-33333333333333333","VolumeSize":1,"State":"error"}
            ],"nextToken":"SECRETTOKEN","OwnerId":"123456789012"}"#);
        assert_eq!(snaps.snapshots[0].tier, SnapshotTier::Standard);
        assert_eq!(snaps.snapshots[0].state, SnapshotState::Completed);
        assert!(snaps.snapshots[0].start_ms.is_some());
        assert_eq!(snaps.snapshots[1].full_bytes, Some(1_073_741_824));
        assert_eq!(snaps.snapshots[1].tier, SnapshotTier::Archive);
        assert_eq!(snaps.snapshots[1].state, SnapshotState::Pending);
        assert_eq!(snaps.snapshots[2].state, SnapshotState::Error);
        assert_eq!(snaps.warnings.len(), 1);
        assert!(snaps.warnings[0].contains("nextToken was not followed"));
        assert!(!snaps.warnings[0].contains("SECRETTOKEN"));
        assert!(!format!("{snaps:?}").contains("SECRETKEY"));
        assert!(!format!("{snaps:?}").contains("123456789012"));
        let images = doc(
            r#"{"Images":[{"ImageId":"ami-11111111111111111","Description":"CreateImage SECRETKEY","BlockDeviceMappings":[
                {"DeviceName":"/dev/sda1","Ebs":{"SnapshotId":"snap-22222222222222222"}},
                {"DeviceName":"/dev/sdb","VirtualName":"ephemeral0"}
            ]}]}"#,
        );
        assert_eq!(
            images.images[0].snapshot_ids,
            vec!["snap-22222222222222222".to_string()]
        );
        assert!(images.saw_images);
        assert!(!format!("{images:?}").contains("SECRETKEY"));
    }

    #[test]
    fn empty_volumes_mixed_document_and_duplicate_key() {
        let empty = doc(r#"{"Volumes":[]}"#);
        assert!(empty.saw_volumes);
        assert!(empty.volumes.is_empty());
        let mixed = parse_document(
            "f.json",
            br#"{"Volumes":[],"Snapshots":[],"OwnerId":"SECRET"}"#,
            true,
        )
        .unwrap_err();
        assert_eq!(mixed.message(), "f.json: mixed document");
        assert!(!mixed.message().contains("SECRET"));
        let dup = parse_document(
            "f.json",
            br#"{"Volumes":[],"Volumes":[{"VolumeId":"SECRET"}]}"#,
            true,
        )
        .unwrap_err();
        assert_eq!(dup.message(), "f.json: duplicate key");
        assert!(!dup.message().contains("SECRET"));
        let bad_json = parse_document("f.json", br#"{"note":"SECRETJSON""#, true).unwrap_err();
        assert_eq!(bad_json.message(), "f.json: not JSON");
        assert!(!bad_json.message().contains("SECRETJSON"));
    }

    #[test]
    fn gzip_round_trip_and_bad_snapshot_id_on_an_image() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;
        let json = br#"{"Volumes":[]}"#;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(json).unwrap();
        let bytes = encoder.finish().unwrap();
        let parsed = parse_document("in.gz", &bytes, true).unwrap();
        assert!(parsed.saw_volumes);
        let err = parse_document(
            "f.json",
            br#"{"ImageId":"ami-11111111111111111","BlockDeviceMappings":[{"Ebs":{"SnapshotId":"SECRETKEY"}}]}"#,
            true,
        )
        .unwrap_err();
        assert_eq!(
            err.message(),
            "f.json: ami-11111111111111111: bad SnapshotId"
        );
        assert!(!err.message().contains("SECRETKEY"));
    }

    #[test]
    fn missing_state_is_completed_and_other_tier_is_other() {
        let parsed =
            doc(r#"{"SnapshotId":"snap-0123456789abcdef0","VolumeSize":2,"StorageTier":"SECRET"}"#);
        assert_eq!(parsed.snapshots[0].state, SnapshotState::Completed);
        assert_eq!(parsed.snapshots[0].tier, SnapshotTier::Other);
        assert!(parsed.snapshots[0].start_ms.is_none());
    }

    #[test]
    fn array_of_one_kind_and_a_future_style_decimal_size() {
        let parsed = doc(r#"[{"VolumeId":"vol-01234567","Size":3.0,"VolumeType":"st1"}]"#);
        assert_eq!(parsed.volumes[0].size_gb, 3);
        assert_eq!(parsed.volumes[0].volume_type, "st1");
        let err = parse_document("f.json", b"[]", true).unwrap_err();
        assert_eq!(
            err.message(),
            "f.json: expected volumes, snapshots, or images"
        );
    }

    #[test]
    fn describe_output_keeps_volume_id_and_empty_snapshot_id() {
        let snap = doc(
            r#"{"Snapshots":[{"SnapshotId":"snap-0123456789abcdef0","StartTime":"2026-01-01T00:00:00+00:00","State":"completed","VolumeId":"vol-0123456789abcdef0","VolumeSize":8,"StorageTier":"standard"}]}"#,
        );
        assert!(snap.saw_snapshots);
        assert_eq!(snap.snapshots.len(), 1);
        assert_eq!(snap.snapshots[0].volume_size_gb, Some(8));
        let vol = doc(
            r#"{"Volumes":[{"VolumeId":"vol-0123456789abcdef0","Size":8,"VolumeType":"gp2","SnapshotId":"","Attachments":[]}]}"#,
        );
        assert!(vol.saw_volumes);
        assert_eq!(vol.volumes[0].size_gb, 8);
        assert!(vol.volumes[0].loose);
        let bare = doc(
            r#"[{"SnapshotId":"snap-0123456789abcdef0","VolumeId":"vol-0123456789abcdef0","VolumeSize":8}]"#,
        );
        assert!(bare.saw_snapshots);
        assert_eq!(bare.snapshots[0].id, "snap-0123456789abcdef0");
        let inside = parse_document(
            "f.json",
            br#"{"Volumes":[{"SnapshotId":"snap-0123456789abcdef0","VolumeId":"vol-0123456789abcdef0","VolumeSize":8}]}"#,
            true,
        )
        .unwrap_err();
        assert_eq!(inside.message(), "f.json: mixed document");
    }

    #[test]
    fn every_parse_error_text_is_pinned() {
        let cases: [(&str, &str); 15] = [
            (r#"{"Volumes":[],"nextToken":5}"#, "f.json: bad nextToken"),
            (r#"{"Volumes":5}"#, "f.json: bad Volumes"),
            (
                r#"{"VolumeId":"vol-01234567","Size":1}"#,
                "f.json: vol-01234567: bad VolumeType",
            ),
            (
                r#"{"VolumeId":"vol-01234567","VolumeType":"gp2"}"#,
                "f.json: vol-01234567: missing Size",
            ),
            (
                r#"{"VolumeId":"vol-01234567","VolumeType":"gp2","Size":"x"}"#,
                "f.json: vol-01234567: bad Size",
            ),
            (
                r#"{"VolumeId":"vol-01234567","VolumeType":"gp2","Size":1,"State":5}"#,
                "f.json: vol-01234567: bad State",
            ),
            (
                r#"{"VolumeId":"vol-01234567","VolumeType":"gp2","Size":1,"AvailabilityZone":5}"#,
                "f.json: vol-01234567: bad AvailabilityZone",
            ),
            (
                r#"{"VolumeId":"vol-01234567","VolumeType":"gp2","Size":1,"Attachments":5}"#,
                "f.json: vol-01234567: bad Attachments",
            ),
            (
                r#"{"VolumeId":"vol-01234567","VolumeType":"gp2","Size":1,"Attachments":[1]}"#,
                "f.json: vol-01234567: bad Attachments",
            ),
            (
                r#"{"SnapshotId":"snap-01234567","StartTime":"2026-01-01T00:00:00Z"}"#,
                "f.json: snap-01234567: missing size",
            ),
            (
                r#"{"SnapshotId":"snap-01234567","VolumeSize":1,"StorageTier":5}"#,
                "f.json: snap-01234567: bad StorageTier",
            ),
            (
                r#"{"SnapshotId":"snap-01234567","VolumeSize":1,"State":5}"#,
                "f.json: snap-01234567: bad State",
            ),
            (
                r#"{"ImageId":"ami-01234567","BlockDeviceMappings":5}"#,
                "f.json: ami-01234567: bad BlockDeviceMappings",
            ),
            (
                r#"{"ImageId":"ami-01234567","BlockDeviceMappings":[1]}"#,
                "f.json: ami-01234567: bad BlockDeviceMappings",
            ),
            (
                r#"{"ImageId":"ami-01234567","BlockDeviceMappings":[{"Ebs":5}]}"#,
                "f.json: ami-01234567: bad BlockDeviceMappings",
            ),
        ];
        for (json, message) in cases {
            let err = parse_document("f.json", json.as_bytes(), true).unwrap_err();
            assert_eq!(err.message(), message, "{json}");
        }
        let gzip = parse_document("f.json", &[0x1f, 0x8b, 0x00, 0x00], true).unwrap_err();
        assert_eq!(gzip.message(), "f.json: could not read gzip");
    }
}
