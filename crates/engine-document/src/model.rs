use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{Result, invalid};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_IMAGE_SIZE: u32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetId(pub u64);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PngAsset {
    pub id: AssetId,
    /// 프로젝트 디렉터리를 기준으로 하는 슬래시 구분 상대 경로입니다.
    pub path: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpriteEntity {
    pub id: EntityId,
    pub asset_id: AssetId,
    pub name: String,
    pub position: Position,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    /// 목록 순서가 그리기 순서입니다. 삭제를 취소하면 원래 순서도 복원합니다.
    pub entities: Vec<SpriteEntity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectDocument {
    pub schema_version: u32,
    pub name: String,
    pub logical_width: u32,
    pub logical_height: u32,
    pub scene: Scene,
    pub assets: Vec<PngAsset>,
}

impl ProjectDocument {
    pub(crate) fn new(name: &str, width: u32, height: u32) -> Result<Self> {
        let document = Self {
            schema_version: SCHEMA_VERSION,
            name: name.to_owned(),
            logical_width: width,
            logical_height: height,
            scene: Scene::default(),
            assets: Vec::new(),
        };
        document.validate()?;
        Ok(document)
    }

    /// 파일을 열거나 편집 명령을 적용할 때 같은 규칙으로 검증합니다.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(invalid("지원하지 않는 schema_version입니다."));
        }
        validate_name(&self.name)?;
        validate_dimensions(self.logical_width, self.logical_height)?;

        let mut assets = HashSet::new();
        let mut paths = HashSet::new();
        for asset in &self.assets {
            if asset.id.0 == 0 || !assets.insert(asset.id) {
                return Err(invalid("리소스 ID는 0이 아니며 서로 달라야 합니다."));
            }
            validate_asset_path(&asset.path)?;
            if !paths.insert(&asset.path) {
                return Err(invalid("리소스 경로가 중복되었습니다."));
            }
            validate_dimensions(asset.width, asset.height)?;
        }

        let mut entities = HashSet::new();
        for entity in &self.scene.entities {
            if entity.id.0 == 0 || !entities.insert(entity.id) {
                return Err(invalid("개체 ID는 0이 아니며 서로 달라야 합니다."));
            }
            if !assets.contains(&entity.asset_id) {
                return Err(invalid("개체가 문서에 없는 리소스를 참조합니다."));
            }
            validate_name(&entity.name)?;
        }
        Ok(())
    }
}

pub(crate) fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty() || name.chars().count() > 120 {
        return Err(invalid(
            "이름은 공백만으로 구성할 수 없으며 120자 이하여야 합니다.",
        ));
    }
    Ok(())
}

pub(crate) fn validate_dimensions(width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 || width > MAX_IMAGE_SIZE || height > MAX_IMAGE_SIZE {
        return Err(invalid("가로·세로 크기는 각각 1~4096 픽셀이어야 합니다."));
    }
    Ok(())
}

pub(crate) fn validate_asset_path(path: &str) -> Result<()> {
    let components: Vec<_> = path.split('/').collect();
    if components.len() != 2
        || components[0] != "assets"
        || components[1].is_empty()
        || components[1] == "."
        || components[1] == ".."
        || path.contains(['\\', ':'])
        || !components[1].ends_with(".png")
    {
        return Err(invalid("PNG 경로는 assets/파일.png 형식이어야 합니다."));
    }
    Ok(())
}
