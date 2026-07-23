use base64::Engine as _;

const PNG_DATA_PREFIX: &str = "data:image/png;base64,";
const PNG_BASE64_HEADER_CHARS: usize = 32;
const BASE64_QUANTUM_CHARS: usize = 4;
const PNG_REQUIRED_HEADER_BYTES: usize = 24;
const PNG_SIGNATURE_BYTES: usize = 8;
const PNG_WIDTH_START: usize = 16;
const PNG_WIDTH_END: usize = 20;
const PNG_HEIGHT_START: usize = 20;
const PNG_HEIGHT_END: usize = 24;

pub(super) fn data_dimensions(source: &str) -> Option<(f32, f32)> {
    let encoded = source.strip_prefix(PNG_DATA_PREFIX)?;
    let header_end = encoded.len().min(PNG_BASE64_HEADER_CHARS);
    let header_end = header_end - header_end % BASE64_QUANTUM_CHARS;
    let header = base64::engine::general_purpose::STANDARD
        .decode(&encoded[..header_end])
        .ok()?;
    if header.len() < PNG_REQUIRED_HEADER_BYTES
        || &header[..PNG_SIGNATURE_BYTES] != b"\x89PNG\r\n\x1a\n"
    {
        return None;
    }
    let width = u32::from_be_bytes(header[PNG_WIDTH_START..PNG_WIDTH_END].try_into().ok()?) as f32;
    let height =
        u32::from_be_bytes(header[PNG_HEIGHT_START..PNG_HEIGHT_END].try_into().ok()?) as f32;
    (width > 0.0 && height > 0.0).then_some((width, height))
}

#[cfg(test)]
mod tests {
    use super::data_dimensions;

    #[test]
    fn png_dimensions_reject_truncated_or_non_png_data() {
        assert!(data_dimensions("data:image/png;base64,AAAA").is_none());
        assert!(data_dimensions("data:image/png;base64,SGVsbG8sIHdvcmxkIQ==").is_none());
    }
}
