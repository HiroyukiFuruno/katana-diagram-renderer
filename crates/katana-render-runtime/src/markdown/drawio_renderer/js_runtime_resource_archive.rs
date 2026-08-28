use super::{DrawioResource, drawio_resource, selector::DrawioResourceSelector};
use crate::markdown::runtime_asset_archive::RuntimeAssetArchive;
use std::sync::OnceLock;

const DRAWIO_RESOURCE_ARCHIVE: &[u8] = include_bytes!("generated/drawio-resources.bin.br");
static DRAWIO_RESOURCE_ARCHIVE_BYTES: OnceLock<Result<Vec<u8>, String>> = OnceLock::new();

pub(super) type DrawioResourceArchiveEntry = (&'static str, usize, usize);
pub(super) type DrawioResourceArchiveIndex = &'static [DrawioResourceArchiveEntry];

include!("generated/drawio-resources-index.rs");

pub(super) struct DrawioResourceArchive;

impl DrawioResourceArchive {
    pub(super) fn collect(
        selector: &DrawioResourceSelector<'_>,
        resources: &mut Vec<DrawioResource>,
    ) -> Result<(), String> {
        let archive = Self::bytes()?;
        for index in DRAWIO_RESOURCE_ARCHIVE_INDEXES {
            Self::collect_index(archive, index, selector, resources)?;
        }
        Ok(())
    }

    fn collect_index(
        archive: &[u8],
        index: DrawioResourceArchiveIndex,
        selector: &DrawioResourceSelector<'_>,
        resources: &mut Vec<DrawioResource>,
    ) -> Result<(), String> {
        for &(path, start, length) in index {
            if selector.includes(path) {
                resources.push(drawio_resource(
                    path.to_string(),
                    resource_contents(archive, path, start, length)?,
                ));
            }
        }
        Ok(())
    }

    fn bytes() -> Result<&'static [u8], String> {
        DRAWIO_RESOURCE_ARCHIVE_BYTES
            .get_or_init(|| {
                validate_resource_archive(
                    RuntimeAssetArchive::brotli(DRAWIO_RESOURCE_ARCHIVE)?,
                    DRAWIO_RESOURCE_ARCHIVE_UNCOMPRESSED_LENGTH,
                )
            })
            .as_ref()
            .map(Vec::as_slice)
            .map_err(Clone::clone)
    }
}

pub(super) fn validate_resource_archive(
    bytes: Vec<u8>,
    expected_length: usize,
) -> Result<Vec<u8>, String> {
    if bytes.len() != expected_length {
        return Err(format!(
            "Draw.io resource archive length mismatch: expected {expected_length}, got {}",
            bytes.len()
        ));
    }
    Ok(bytes)
}

pub(super) fn resource_contents<'a>(
    archive: &'a [u8],
    path: &str,
    start: usize,
    length: usize,
) -> Result<&'a [u8], String> {
    let Some(end) = start.checked_add(length) else {
        return Err(format!("Draw.io resource archive offset overflow: {path}"));
    };
    archive
        .get(start..end)
        .ok_or_else(|| format!("Draw.io resource archive entry is out of bounds: {path}"))
}
