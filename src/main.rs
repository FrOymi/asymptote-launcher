use reqwest::Client;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::fs::{create_dir_all, File};
use tokio::io::AsyncWriteExt;

// Структуры
#[derive(Deserialize)]
struct VersionManifest {
    latest: Latest,
    versions: Vec<Version>,
}

#[derive(Deserialize)]
struct Latest {
    release: String,
}

#[derive(Deserialize)]
struct Version {
    id: String,
    url: String,
}

struct MinecraftManifest {
    id: String,
    url: String,
}


// Основная функция скрипта
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let http_client = Client::new();
    let version_manifest_url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

    if let Some(base_dir) = get_minecraft_dir() {
        let manifest = fetch_version_manifest(&http_client, version_manifest_url).await?;
        let latest_release = find_latest_release(&manifest)
            .ok_or("Failed to find the latest release in the manifest")?;

        let path = download_version_json(&http_client, &latest_release, &base_dir).await?;
        println!("Version file saved to: {:?}", path);

    } else { println!("Failed to determine the path to system folders.") }

    Ok(())
}

// Получаем путь к директории .minecraft и создаем папку для сохранения Манифеста версий
fn get_minecraft_dir() -> Option<PathBuf> {
    let mut path = dirs::data_dir()?;

    match fs::create_dir_all(
        path
            .join(".minecraft")
            .join("versions")
    ) {
        Ok(_) => {
            path.push(".minecraft");
            Some(path)
        },
        Err(e) => {
            eprintln!("Failed to create folder: {}", e);
            None
        }
    }
}


// Получаем список Манифестов всех достпуных версий Minecraft
async fn fetch_version_manifest(
    http_client: &Client,
    version_manifest_url: &str,
) -> Result<VersionManifest, reqwest::Error> {

    let manifest: VersionManifest = http_client
        .get(version_manifest_url)
        .send()
        .await?
        .json()
        .await?;

    Ok(manifest)
}


// Тестовая функция (В будущем может быть заменена на ручной выбор версии)
// Получаем <version>.json последнего релиза Minecraft
fn find_latest_release(
    manifest: &VersionManifest,
) -> Option<MinecraftManifest> {
    let latest_release = &manifest.latest.release;
    let target_version = manifest.versions
        .iter()
        .find(|version| version.id.eq(latest_release))?;

    println!("latest version: {}", latest_release);

    Some(MinecraftManifest {
        id: latest_release.clone(),
        url: target_version.url.clone(),
    })
}


// Устанавливаем <version>.json
async fn download_version_json(
    http_client: &Client,
    minecraft_manifest: &MinecraftManifest,
    base_dir: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let version_dir = base_dir.join("versions").join(&minecraft_manifest.id);

    create_dir_all(&version_dir).await?;

    let file_path = version_dir.join(format!("{}.json", minecraft_manifest.id));

    let response_text = http_client
        .get(&minecraft_manifest.url)
        .send()
        .await?
        .text()
        .await?;

    let mut file = File::create(&file_path).await?;
    file.write_all(response_text.as_bytes()).await?;

    Ok(file_path)
}