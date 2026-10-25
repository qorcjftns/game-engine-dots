use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use crate::{
    AssetId, DocumentError, EntityId, PngAsset, PngPixels, Position, ProjectDocument, Result,
    SpriteEntity, assets::read_png, invalid, storage,
};

const HISTORY_LIMIT: usize = 128;

#[derive(Debug, Clone)]
pub enum EditCommand {
    AddSprite {
        asset_id: AssetId,
        name: String,
        position: Position,
    },
    RenameEntity {
        id: EntityId,
        name: String,
    },
    MoveEntity {
        id: EntityId,
        position: Position,
    },
    RemoveEntity {
        id: EntityId,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct CommandResult {
    pub entity_id: Option<EntityId>,
}

/// 문서를 수정하는 공통 경로입니다. 선택·드래그 미리보기는 GUI가 보관합니다.
#[derive(Debug)]
pub struct EditorSession {
    project_file: PathBuf,
    document: ProjectDocument,
    saved_document: ProjectDocument,
    saved_bytes: Vec<u8>,
    undo: Vec<ProjectDocument>,
    redo: Vec<ProjectDocument>,
    next_entity_id: Option<u64>,
    next_asset_id: Option<u64>,
}

impl EditorSession {
    /// 빈 프로젝트를 만들고 저장합니다. 기존 파일을 덮어쓰지 않습니다.
    pub fn create(
        project_file: impl AsRef<Path>,
        name: &str,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let document = ProjectDocument::new(name, width, height)?;
        let bytes = storage::document_bytes(&document)?;
        let project_file = storage::new_project_path(project_file.as_ref())?;
        storage::atomic_write(&project_file, &bytes, true)?;
        Ok(Self::from_document(project_file, document, bytes))
    }

    /// 성공한 경우에만 반환된 세션으로 GUI의 현재 세션을 교체하세요.
    pub fn open(project_file: impl AsRef<Path>) -> Result<Self> {
        let (project_file, document, bytes) = storage::read_project(project_file.as_ref())?;
        Ok(Self::from_document(project_file, document, bytes))
    }

    fn from_document(project_file: PathBuf, document: ProjectDocument, bytes: Vec<u8>) -> Self {
        let next_entity_id = document
            .scene
            .entities
            .iter()
            .map(|entity| entity.id.0)
            .max()
            .unwrap_or(0)
            .checked_add(1);
        let next_asset_id = document
            .assets
            .iter()
            .map(|asset| asset.id.0)
            .max()
            .unwrap_or(0)
            .checked_add(1);
        Self {
            project_file,
            saved_document: document.clone(),
            document,
            saved_bytes: bytes,
            undo: Vec::new(),
            redo: Vec::new(),
            next_entity_id,
            next_asset_id,
        }
    }

    pub fn document(&self) -> &ProjectDocument {
        &self.document
    }

    pub fn project_file(&self) -> &Path {
        &self.project_file
    }

    pub fn project_directory(&self) -> &Path {
        // create/open에서 항상 부모가 있는 절대 경로를 만듭니다.
        self.project_file
            .parent()
            .expect("project path has a parent")
    }

    /// 이력 위치 대신 저장한 문서와 비교하므로 저장 지점으로 undo하면 다시 clean이 됩니다.
    pub fn is_dirty(&self) -> bool {
        self.document != self.saved_document
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn execute(&mut self, command: EditCommand) -> Result<CommandResult> {
        let mut document = self.document.clone();
        let id = match command {
            EditCommand::AddSprite {
                asset_id,
                name,
                position,
            } => {
                let id = EntityId(
                    self.next_entity_id
                        .ok_or_else(|| invalid("개체 ID가 소진되었습니다."))?,
                );
                document.scene.entities.push(SpriteEntity {
                    id,
                    asset_id,
                    name,
                    position,
                });
                id
            }
            EditCommand::RenameEntity { id, name } => {
                entity_mut(&mut document, id)?.name = name;
                id
            }
            EditCommand::MoveEntity { id, position } => {
                entity_mut(&mut document, id)?.position = position;
                id
            }
            EditCommand::RemoveEntity { id } => {
                let index = document
                    .scene
                    .entities
                    .iter()
                    .position(|entity| entity.id == id)
                    .ok_or_else(|| invalid("개체를 찾을 수 없습니다."))?;
                document.scene.entities.remove(index);
                id
            }
        };
        let added = document.scene.entities.len() > self.document.scene.entities.len();
        self.commit(document)?;
        if added {
            self.next_entity_id = id.0.checked_add(1);
        }
        Ok(CommandResult {
            entity_id: Some(id),
        })
    }

    fn commit(&mut self, document: ProjectDocument) -> Result<()> {
        // 사본을 검증한 후에만 문서와 이력을 바꿉니다. 무효 명령은 상태를 남기지 않습니다.
        document.validate()?;
        if document == self.document {
            return Ok(());
        }
        self.undo
            .push(std::mem::replace(&mut self.document, document));
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        Ok(())
    }

    /// 문서 전체의 작은 스냅샷으로 이전 값·개체·삽입 순서를 함께 복원합니다.
    pub fn undo(&mut self) -> bool {
        let Some(document) = self.undo.pop() else {
            return false;
        };
        self.redo
            .push(std::mem::replace(&mut self.document, document));
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(document) = self.redo.pop() else {
            return false;
        };
        self.undo
            .push(std::mem::replace(&mut self.document, document));
        true
    }

    /// 저장 실패 시 파일·문서·저장 지점·undo/redo를 그대로 유지합니다.
    pub fn save(&mut self) -> Result<()> {
        let current_bytes = fs::read(&self.project_file)?;
        if current_bytes != self.saved_bytes {
            return Err(DocumentError::ExternallyModified);
        }
        let bytes = storage::document_bytes(&self.document)?;
        storage::atomic_write(&self.project_file, &bytes, false)?;
        self.saved_document = self.document.clone();
        self.saved_bytes = bytes;
        Ok(())
    }

    /// 전체 PNG 디코딩에 성공한 파일만 가져옵니다. 등록과 개체 추가는 별도 명령 단위입니다.
    pub fn import_png(&mut self, source: impl AsRef<Path>) -> Result<AssetId> {
        let (bytes, pixels) = read_png(source.as_ref())?;
        let directory = self.project_directory().join("assets");
        fs::create_dir_all(&directory)?;
        let directory = directory.canonicalize()?;
        if !directory.starts_with(self.project_directory()) {
            return Err(invalid("리소스 디렉터리가 프로젝트 외부를 가리킵니다."));
        }

        let mut number = self
            .next_asset_id
            .ok_or_else(|| invalid("리소스 ID가 소진되었습니다."))?;
        let (id, destination) = loop {
            let path = directory.join(format!("asset-{number}.png"));
            let relative_path = format!("assets/asset-{number}.png");
            let referenced = self
                .document
                .assets
                .iter()
                .any(|asset| asset.path == relative_path);
            if !path.exists() && !referenced {
                break (AssetId(number), path);
            }
            number = number
                .checked_add(1)
                .ok_or_else(|| invalid("리소스 ID가 소진되었습니다."))?;
        };
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)?;
        if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(&destination);
            return Err(error.into());
        }
        drop(file);

        let mut document = self.document.clone();
        document.assets.push(PngAsset {
            id,
            path: format!("assets/asset-{}.png", id.0),
            width: pixels.width,
            height: pixels.height,
        });
        if let Err(error) = self.commit(document) {
            let _ = fs::remove_file(&destination);
            return Err(error);
        }
        self.next_asset_id = id.0.checked_add(1);
        Ok(id)
    }

    /// canonical 경로가 프로젝트 안에 있는지 검사한 뒤 GUI에 실제 경로를 제공합니다.
    pub fn asset_path(&self, id: AssetId) -> Result<PathBuf> {
        let asset = self
            .document
            .assets
            .iter()
            .find(|asset| asset.id == id)
            .ok_or_else(|| invalid("리소스를 찾을 수 없습니다."))?;
        let path = self.project_directory().join(&asset.path).canonicalize()?;
        if !path.starts_with(self.project_directory()) {
            return Err(invalid("리소스 파일이 프로젝트 외부를 가리킵니다."));
        }
        Ok(path)
    }

    pub fn read_png_asset(&self, id: AssetId) -> Result<PngPixels> {
        let (_, pixels) = read_png(&self.asset_path(id)?)?;
        let asset = self
            .document
            .assets
            .iter()
            .find(|asset| asset.id == id)
            .ok_or_else(|| invalid("리소스를 찾을 수 없습니다."))?;
        if pixels.width != asset.width || pixels.height != asset.height {
            return Err(invalid("PNG 크기가 저장된 리소스 정보와 다릅니다."));
        }
        Ok(pixels)
    }
}

fn entity_mut(document: &mut ProjectDocument, id: EntityId) -> Result<&mut SpriteEntity> {
    document
        .scene
        .entities
        .iter_mut()
        .find(|entity| entity.id == id)
        .ok_or_else(|| invalid("개체를 찾을 수 없습니다."))
}
