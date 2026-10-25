use std::{io::Cursor, path::Path};

use crate::{Result, invalid, model::validate_dimensions, storage::read_limited};

const MAX_PNG_BYTES: u64 = 16 * 1024 * 1024;

/// UI 라이브러리와 관계없는 RGBA8 픽셀입니다. GPU 업로드·캐시는 GUI가 담당합니다.
#[derive(Debug)]
pub struct PngPixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub(crate) fn read_png(path: &Path) -> Result<(Vec<u8>, PngPixels)> {
    let bytes = read_limited(path, MAX_PNG_BYTES)?;
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(invalid("PNG 파일만 가져올 수 있습니다."));
    }
    let reader = image::ImageReader::with_format(Cursor::new(&bytes), image::ImageFormat::Png);
    let (width, height) = reader.into_dimensions()?;
    validate_dimensions(width, height)?;
    // 확장자와 헤더만 확인하지 않고 실제 픽셀까지 디코딩합니다.
    let rgba = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)?.to_rgba8();
    Ok((
        bytes,
        PngPixels {
            width,
            height,
            rgba: rgba.into_raw(),
        },
    ))
}
