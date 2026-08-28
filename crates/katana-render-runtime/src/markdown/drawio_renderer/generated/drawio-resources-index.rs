pub(super) const DRAWIO_RESOURCE_ARCHIVE_UNCOMPRESSED_LENGTH: usize = 1020564;
include!("drawio-resources-media-index.rs");
include!("drawio-resources-data-index.rs");
pub(super) const DRAWIO_RESOURCE_ARCHIVE_INDEXES: &[DrawioResourceArchiveIndex] = &[
    DRAWIO_RESOURCE_ARCHIVE_MEDIA_INDEX,
    DRAWIO_RESOURCE_ARCHIVE_DATA_INDEX,
];
