use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
struct VersionManifest {
    latest: Latest,
    versions: Vec<Version>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Latest {
    release: String,
    snapshot: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Version {
    id: String,
    r#type: String,
    url: String,
}

#[tauri::command]
pub async fn get_minecraft_versions() -> Result<Vec<Version>, String> {
    let url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

    let manifest: VersionManifest = reqwest::get(url)
        .await
        .map_err(|e| format!("{}", e))?
        .json()
        .await
        .map_err(|e| format!("{}", e))?;

    Ok(manifest.versions)
}