use std::path::{Path, PathBuf};
use std::fs;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct VersionManifest {
    latest: Latest,
    versions: Vec<Version>,
}

#[derive(Debug, Deserialize)]
struct Latest {
    release: String,
    snapshot: String,
}

#[derive(Deserialize, Debug)]
struct Version {
    id: String,
    r#type: String,
    url: String,
}

#[derive(Deserialize, Debug)]
struct ClientJson {
    id: String,
    downloads: Downloads,
}

#[derive(Deserialize, Debug)]
struct Downloads {
    client: ClientJar,
}

#[derive(Deserialize, Debug)]
struct ClientJar {
    sha1: String,
    size: u64,
    url: String,
}






#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let http_client = Client::new();


    if let Some(base_dir) = get_minecraft_dir() {
        let folder_to_create = ["versions", "libraries"];

        for folder in &folder_to_create {
            let full_path = base_dir.join(folder);
            fs::create_dir_all(&full_path)?;
            println!("Папка {} успешно проверена/создана по пути: {:?}", folder, full_path);
        }

        let url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

        println!("Получаем последнюю стабильную версию Minecraft...");

        let latest_url = get_last_version(&http_client, url, &base_dir).await?;

        read_client_manifest(&http_client, latest_url, base_dir).await?;

    } else { eprintln!("Не удалось определить путь к системным папкам") }

    Ok(())
}

fn get_minecraft_dir() -> Option<PathBuf> {
    let mut path = dirs::data_dir()?;
    path.push(".minecraft");
    Some(path)
}

async fn get_last_version(
    client: &Client,
    version_url: &str,
    base_dir: &Path,
) -> Result<String, Box<dyn std::error::Error>> {

    let manifest: VersionManifest = client
        .get(version_url)
        .send()
        .await?
        .json()
        .await?;

    if !manifest.versions.is_empty() {
        let url_latest_release = manifest.versions
            .iter()
            .find(|version| version.id == manifest.latest.release)
            .map(|version| &version.url);

        println!("Последняя стабильная версия: {}", manifest.latest.release);

        if let Some(url) = url_latest_release {
            println!("{}", url);

            let response_text = client
                .get(url)
                .send()
                .await?
                .text()
                .await?;

            let manifest_file_path = base_dir
                .join("versions")
                .join(&manifest.latest.release);

            fs::create_dir_all(&manifest_file_path)?;

            let mut file = File::create(
                manifest_file_path.join(format!("{}.json", manifest.latest.release))
            ).await?;

            file.write_all(response_text.as_bytes()).await?;

            println!("{}.json был сохранен", manifest.latest.release);
            return Ok(url.clone());
        }
    }

    Err("Не удалось найти URL для последней версии".into())
}

async fn read_client_manifest(
    client: &Client,
    client_manifest_url: String,
    base_dir: PathBuf
) -> Result<(), Box<dyn std::error::Error>> {

    println!("Чтение Client.json...");

    let client_json: ClientJson = client
        .get(client_manifest_url)
        .send()
        .await?
        .json()
        .await?;

    let version_dir = base_dir.join("versions").join(&client_json.id);
    fs::create_dir_all(&version_dir)?;

    let client_download_url = client_json.downloads.client.url;

    println!("{}.jar: {}",client_json.id, client_download_url);

    let file_path = version_dir.join(format!("{}.jar", client_json.id));

    if !file_path.exists() {
        let response = client
            .get(client_download_url)
            .send()
            .await?;
        let mut dest = File::create(&file_path).await?;

        let bytes = response.bytes().await?;
        dest.write_all(&bytes).await?;

        println!("{}.jar успешно скачан в: {:?}", client_json.id, file_path);
    } else { println!("{}.jar уже существует в: {:?}", client_json.id, file_path) }

    Ok(())
}

