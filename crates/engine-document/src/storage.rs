use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::{DocumentError, ProjectDocument, Result, invalid};

const MAX_DOCUMENT_BYTES: u64 = 2 * 1024 * 1024;

pub(crate) fn new_project_path(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let name = absolute
        .file_name()
        .ok_or_else(|| invalid("프로젝트 파일 이름이 필요합니다."))?;
    let parent = absolute
        .parent()
        .ok_or_else(|| invalid("프로젝트 디렉터리가 필요합니다."))?;
    fs::create_dir_all(parent)?;
    Ok(parent.canonicalize()?.join(name))
}

pub(crate) fn read_project(path: &Path) -> Result<(PathBuf, ProjectDocument, Vec<u8>)> {
    let path = path.canonicalize()?;
    let bytes = read_limited(&path, MAX_DOCUMENT_BYTES)?;
    let mut document: ProjectDocument = serde_json::from_slice(&bytes)?;
    if document.schema_version == 1 {
        // v1은 행동 필드를 정의하지 않았습니다. 확장 필드가 있으면 잘못 표기한 파일입니다.
        if !document.behaviors.is_empty()
            || document
                .scene
                .entities
                .iter()
                .any(|e| e.behavior_id.is_some())
        {
            return Err(invalid("schema_version 1에는 행동을 저장할 수 없습니다."));
        }
        document.schema_version = 2;
    }
    document.validate()?;
    Ok((path, document, bytes))
}

pub(crate) fn document_bytes(document: &ProjectDocument) -> Result<Vec<u8>> {
    document.validate()?;
    let mut bytes = serde_json::to_vec_pretty(document)?;
    bytes.push(b'\n');
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(invalid("프로젝트 JSON은 2MiB 이하여야 합니다."));
    }
    Ok(bytes)
}

/// 임시 파일을 같은 디렉터리에 완전히 기록한 후 교체합니다.
pub(crate) fn atomic_write(path: &Path, bytes: &[u8], create_new: bool) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("프로젝트 디렉터리가 필요합니다."))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    if create_new {
        temporary
            .persist_noclobber(path)
            .map_err(|error| DocumentError::Io(error.error))?;
    } else {
        temporary
            .persist(path)
            .map_err(|error| DocumentError::Io(error.error))?;
    }
    Ok(())
}

pub(crate) fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>> {
    if fs::metadata(path)?.len() > limit {
        return Err(invalid("파일 크기 제한을 초과했습니다."));
    }
    let bytes = fs::read(path)?;
    if bytes.len() as u64 > limit {
        return Err(invalid("파일 크기 제한을 초과했습니다."));
    }
    Ok(bytes)
}
