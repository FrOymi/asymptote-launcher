use std::collections::HashMap;
use reqwest::Client;
use serde::Deserialize;
use std::fs::{self, File as StdFile};
// use std::io;
use std::io::{BufReader as StdBufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs::{create_dir_all, File as TokioFile};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader as TokioBufReader };
use sha1::{Sha1, Digest};
use hex;
// use zip::ZipArchive;

// Структуры version_manifest_v2.json
#[derive(Deserialize)]
struct VersionManifest {
    latest: ManifestLatest,
    versions: Vec<ManifestVersion>,
}

#[derive(Deserialize)]
struct ManifestLatest {
    release: String,
}

#[derive(Deserialize)]
struct ManifestVersion {
    id: String,
    url: String,
}

// Структуры <version>.json
#[derive(Deserialize)]
struct MinecraftManifest {
    id: String,
    downloads: VersionDownload,
    libraries: Vec<LibraryInfo>,
    assetIndex: AssetIndex,
    logging: LoggingInfo,

    mainClass: String,
    javaVersion: JavaVersionInfo,
    arguments: ArgumentsIndex,
}

#[derive(Deserialize)]
struct ArgumentsIndex {
    game: Vec<Argument>,
    jvm: Vec<Argument>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ArgumentValue {
    String(String),
    List(Vec<String>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Argument {
    String(String),
    Conditional {
        rules: Option<Vec<Rules>>,
        value: ArgumentValue,
    }
}

// Структуры assets
#[derive(Deserialize)]
struct AssetIndex {
    id: String,
    sha1: String,
    size: u64,
    totalSize: u64,
    url: String,
}
#[derive(Deserialize)]
struct Assets { objects: HashMap<String, AssetObject> }
#[derive(Deserialize)]
struct AssetObject { hash: String, size: u64 }


//Вспомогательные структуры

#[derive(Deserialize)]
struct DownloadInfo {
    sha1: String,
    size: u64,
    url: String
}
#[derive(Deserialize)]
struct DownloadLibraryInfo {
    path: String,
    #[serde(flatten)]
    info: DownloadInfo,
}

#[derive(Deserialize)]
struct DownloadLoggingInfo {
    id: String,
    #[serde(flatten)]
    info: DownloadInfo,
}

struct VersionInfo { id: String, url: String } // Структура информации о версии для передачи между функциями
#[derive(Deserialize)]
struct VersionDownload { client: DownloadInfo, server: DownloadInfo }
#[derive(Deserialize)]
struct LibraryInfo { downloads: LibraryArtifact, name: String, rules: Option<Vec<Rules>> }
#[derive(Deserialize)]
struct Rules { action: String, os: Option<OsRule> }
#[derive(Deserialize)]
struct OsRule {
    name: Option<String>,
    arch: Option<String>,
}
#[derive(Deserialize)]
struct LibraryArtifact { artifact: DownloadLibraryInfo }
#[derive(Deserialize)]
struct LoggingInfo { client: LoggingClient }
#[derive(Deserialize)]
struct LoggingClient { argument: String, file: DownloadLoggingInfo, r#type: String }
#[derive(Deserialize)]
struct JavaVersionInfo { component: String, majorVersion: u8 }


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
        println!("Version file saved to: {:?}", path.display());

        let minecraft_manifest = read_version_json(&path)?;

        download_minecraft(&http_client, &minecraft_manifest, &base_dir).await?;

        build_minecraft(&minecraft_manifest, &base_dir).await?;

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
) -> Option<VersionInfo> {
    let latest_release = &manifest.latest.release;
    let target_version = manifest.versions
        .iter()
        .find(|version| version.id.eq(latest_release))?;

    println!("latest version: {}", latest_release);

    Some(VersionInfo {
        id: latest_release.clone(),
        url: target_version.url.clone(),
    })
}


// Устанавливаем <version>.json
async fn download_version_json(
    http_client: &Client,
    minecraft_manifest: &VersionInfo,
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

    let mut file = TokioFile::create(&file_path).await?;
    file.write_all(response_text.as_bytes()).await?;

    Ok(file_path)
}


// Читаем <version>.json
fn read_version_json(
    path: &Path,
) -> Result<MinecraftManifest, Box<dyn std::error::Error>> {
    let version_file = StdFile::open(path)?;
    let reader = StdBufReader::new(version_file);
    let minecraft_manifest: MinecraftManifest = serde_json::from_reader(reader)?;

    Ok(minecraft_manifest)
}


// Функции установки
// Главная функция установки
async fn download_minecraft(
    http_client: &Client,
    minecraft_manifest: &MinecraftManifest,
    base_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {

    install_client(&http_client, &minecraft_manifest, &base_dir).await?;
    install_libraries(&http_client, &minecraft_manifest, &base_dir).await?;
    install_assets(&http_client, &minecraft_manifest, &base_dir).await?;
    install_log_config(&http_client, &minecraft_manifest, &base_dir).await?;

    Ok(())
}


// Функция установки файлов
async fn download_file(
    http_client: &Client,
    file_path: &Path,
    download_info: &DownloadInfo,
) -> Result<(), Box<dyn std::error::Error>> {
    let file_name_os = file_path.file_name().ok_or("Failed to get file name")?;
    let file_name = file_name_os.to_string_lossy();

    let mut attempts = 3;
    while attempts > 0 {
        if !file_path.exists() {
            let response = http_client
                .get(&download_info.url)
                .send()
                .await?;

            if let Some(parent) = file_path.parent() {
                create_dir_all(parent).await?;
            }

            let bytes = response.bytes().await?;

            // Считаем хэш файла
            let mut hasher = Sha1::new();
            hasher.update(&bytes);
            let file_sha1 = hex::encode(hasher.finalize());

            if bytes.len() as u64 == download_info.size && file_sha1 == download_info.sha1 {
                let mut dest = TokioFile::create(&file_path).await?;
                dest.write_all(&bytes).await?;

                println!("Installed {} to: {}", file_name, file_path.display());
                return Ok(());

            } else {
                attempts -= 1;
                if attempts == 0 { return Err(format!("Not installed {}", file_name).into()); }

            }
        } else {
            if download_info.size == fs::metadata(&file_path)?.len() {
                if download_info.sha1 != calculate_file_sha1(&file_path).await? {
                    let _ = tokio::fs::remove_file(&file_path).await;
                    attempts -= 1;
                } else {
                    println!("{} already installed", file_name);
                    return Ok(());
                }
            } else {
                let _ = tokio::fs::remove_file(&file_path).await;
                attempts -= 1;
            }
        }
    }
    Err(format!("Failed to download {}", file_path.display()).into())
}

async fn calculate_file_sha1(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let file = TokioFile::open(&path).await?;
    let mut reader = TokioBufReader::new(file);
    let mut hasher = Sha1::new();

    let mut buffer = [0; 8192];

    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(hex::encode(hasher.finalize()))
}


// Распаковщик
// fn unzipping_natives(
//     file_path: &Path,
//     out_dir: &Path,
// ) -> Result<(), Box<dyn std::error::Error>> {
//     let file = fs::File::open(file_path)?;
//     let mut archive = ZipArchive::new(file)?;
//
//     for i in 0..archive.len() {
//         let mut file = archive.by_index(i)?;
//
//         let out_path = match file.enclosed_name() {
//             Some(path) => out_dir.join(path),
//             None => continue,
//         };
//
//         if file.is_dir() || out_path.starts_with("META-INF") {
//             continue;
//         }
//
//         let is_native = out_path
//             .extension()
//             .and_then(|ext| ext.to_str())
//             .map_or(false, |ext| matches!(ext, "dll" | "so" | "dylib" | "jnilib" ));
//
//         if is_native {
//             let final_path = out_dir.join(&out_path);
//
//             if let Some(parent_dir) = final_path.parent() {
//                 if !parent_dir.exists() {
//                     fs::create_dir_all(parent_dir)?;
//                 }
//             }
//
//             if final_path.exists() {
//                 println!("{} already exists", file.name());
//             } else {
//                 let mut out_file = fs::File::create(&final_path)?;
//                 io::copy(&mut file, &mut out_file)?;
//
//                 println!("Unzipping {} to {}", file.name(), out_path.display());
//             }
//         }
//     }
//
//     Ok(())
// }


// Устанавливаем Client.jar
async fn install_client(
    http_client: &Client,
    minecraft_manifest: &MinecraftManifest,
    base_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let client_dir = base_dir.join("versions");

    let client_file_path = client_dir
        .join(&minecraft_manifest.id)
        .join(format!("{}.jar",&minecraft_manifest.id));

    download_file(&http_client, &client_file_path, &minecraft_manifest.downloads.client).await?;

    Ok(())
}


// Устанавливаем библиотеки
async fn install_libraries(
    http_client: &Client,
    minecraft_manifest: &MinecraftManifest,
    base_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = base_dir.join("libraries");

    let mut os_name = std::env::consts::OS;
    if os_name == "macos" { os_name = "osx"; }

    let arch = std::env::consts::ARCH;

    for lib in minecraft_manifest.libraries.iter() {
        let is_compatible = is_compatible(lib.rules.as_deref(), &os_name, &arch);

        if !is_compatible { continue}

        let library_info = &lib.downloads.artifact;
        let library_path = libraries_dir.join(&library_info.path);

        download_file(&http_client, &library_path, &library_info.info).await?;
    }

    Ok(())
}


// Проверка совместимости
fn is_compatible(
    rules: Option<&[Rules]>,
    os: &str,
    arch: &str,
) -> bool {
    let Some(rules) = rules else {
        return true;
    };

    if rules.is_empty() { return true }

    let mut is_compatible = false;

    for rule in rules {
        let os_matches = match &rule.os {
            None => true,

            Some(os_rule) => {
                let name_matches = os_rule
                    .name
                    .as_deref()
                    .map_or(true, |name| name == os);

                let arch_matches = os_rule
                    .arch
                    .as_deref()
                    .map_or(true, |rule_arch| rule_arch == arch);

                name_matches && arch_matches
            }
        };

        if os_matches { is_compatible = rule.action == "allow"; }
    }

    is_compatible
}


// Устанавливаем ассеты
async fn install_assets(
    http_client: &Client,
    minecraft_manifest: &MinecraftManifest,
    base_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let resources_download_url = "https://resources.download.minecraft.net";

    let asset_json_path = download_assets_json(&http_client, &minecraft_manifest.assetIndex, &base_dir).await?;
    let assets = read_assets_json(&asset_json_path)?;

    let object_path = base_dir.join("assets").join("objects");
    create_dir_all(&object_path).await?;

    for asset in assets.objects.iter() {
        let asset_info = &asset.1;

        let resource_dir_name = &asset_info.hash[..2];
        let resource_path = object_path.join(format!("{}", resource_dir_name)).join(&asset_info.hash);
        let resource_download_url = format!("{}/{}/{}", resources_download_url, resource_dir_name, &asset_info.hash);

        let resource_download_info: DownloadInfo = DownloadInfo {
            sha1: asset_info.hash.clone(),
            size: asset_info.size,
            url: resource_download_url,
        };

        download_file(&http_client, &resource_path, &resource_download_info).await?;
    }

    Ok(())
}

async fn download_assets_json(
    http_client: &Client,
    asset_index: &AssetIndex,
    base_dir: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let asset_dir = base_dir.join("assets").join("indexes");
    create_dir_all(&asset_dir).await?;

    let asset_path = asset_dir.join(format!("{}.json", asset_index.id));

    let response_text = http_client
        .get(&asset_index.url)
        .send()
        .await?
        .text()
        .await?;

    let mut file = TokioFile::create(&asset_path).await?;
    file.write_all(response_text.as_bytes()).await?;

    println!("Assets file saved to: {:?}", asset_path.display());
    Ok(asset_path)
}

fn read_assets_json(
    path: &Path
) -> Result<Assets, Box<dyn std::error::Error>> {
    let file = StdFile::open(path)?;
    let reader = StdBufReader::new(file);
    let assets: Assets = serde_json::from_reader(reader)?;

    Ok(assets)
}


// Устанавливаем конфиг Log4j
async fn install_log_config(
    http_client: &Client,
    minecraft_manifest: &MinecraftManifest,
    base_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let log_config_dir = base_dir.join("assets").join("log-configs");
    let log_config_path = log_config_dir.join(&minecraft_manifest.logging.client.file.id);

    download_file(&http_client, &log_config_path, &minecraft_manifest.logging.client.file.info).await?;

    Ok(())
}



// Собираем конфигурацию запуска Minecraft
async fn build_minecraft(
    minecraft_manifest: &MinecraftManifest,
    base_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let launcher_name = "AsymptoteLauncher";
    let launcher_version = "0.0.5";

    println!("Building Minecraft manifest...");

    let classpath = form_classpath(&minecraft_manifest, &base_dir)?;
    let natives_directory = prepare_natives_directory(&minecraft_manifest, &base_dir).await?;
    let jvm_arguments = form_jvm_arguments(&minecraft_manifest, &base_dir, &launcher_name, &launcher_version, &classpath, &natives_directory);

    Ok(())
}


// Формируем classpath
fn form_classpath(
    minecraft_manifest: &MinecraftManifest,
    base_dir: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    let separator = if cfg!(windows) { ";" } else { ":" };

    let mut libraries_paths: Vec<String> = Vec::new();

    let version_jar_path = base_dir
        .join("versions")
        .join(&minecraft_manifest.id)
        .join(format!("{}.jar", &minecraft_manifest.id))
        .to_string_lossy()
        .into_owned();

    let mut os_name = std::env::consts::OS;
    if os_name == "macos" { os_name = "osx"; }

    let arch = std::env::consts::ARCH;

    for lib in minecraft_manifest.libraries.iter() {
        let is_compatible = is_compatible(lib.rules.as_deref(), &os_name, &arch);

        if !is_compatible { continue; }

        let lib_path = base_dir
            .join("libraries")
            .join(&lib.downloads.artifact.path);
        libraries_paths.push(lib_path.to_string_lossy().into_owned());
    }

    libraries_paths.push(version_jar_path);

    let classpath = libraries_paths.join(separator);

    println!("Classpath successfully created");

    Ok(classpath)
}


// Создание и подготовка директории Natives
async fn prepare_natives_directory(
    minecraft_manifest: &MinecraftManifest,
    base_dir: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let natives_dir = base_dir
        .join("versions")
        .join(&minecraft_manifest.id)
        .join("natives");

    create_dir_all(&natives_dir.join("java")).await?;
    create_dir_all(&natives_dir.join("jna")).await?;
    create_dir_all(&natives_dir.join("lwjgl")).await?;
    create_dir_all(&natives_dir.join("netty")).await?;

    println!("Natives directory successfully created");

    Ok(natives_dir)
}


// Формируем jvm аргументы
fn form_jvm_arguments(
    minecraft_manifest: &MinecraftManifest,
    base_dir: &Path,
    launcher_name: &str,
    launcher_version: &str,
    classpath: &str,
    natives_directory: &Path,
) -> Vec<String> {
    let mut jvm_arguments: Vec<String> = Vec::new();

    let arguments = &minecraft_manifest.arguments;

    let mut os_name = std::env::consts::OS;
    if os_name == "macos" { os_name = "osx"; }

    let arch = std::env::consts::ARCH;

    for argument in arguments.jvm.iter() {
        match argument {
            Argument::String(result) => {
                let processed = result
                    .replace("${launcher_name}", &launcher_name)
                    .replace("${launcher_version}", &launcher_version)
                    .replace("${classpath}", &classpath)
                    .replace("${natives_directory}", &natives_directory.to_string_lossy());
                jvm_arguments.push(processed);
            }

            Argument::Conditional { rules, value } => {
                let is_compatible = is_compatible(rules.as_deref(), &os_name, &arch);

                if !is_compatible { continue; }


                match value {
                    ArgumentValue::String(result) => {
                        let processed = result
                            .replace("${launcher_name}", &launcher_name)
                            .replace("${launcher_version}", &launcher_version)
                            .replace("${classpath}", &classpath)
                            .replace("${natives_directory}", &natives_directory.to_string_lossy());
                        jvm_arguments.push(processed);
                    }

                    ArgumentValue::List(result) => {
                        for result in result {
                            let processed = result
                                .replace("${launcher_name}", &launcher_name)
                                .replace("${launcher_version}", &launcher_version)
                                .replace("${classpath}", &classpath)
                                .replace("${natives_directory}", &natives_directory.to_string_lossy());
                            jvm_arguments.push(processed);
                        }
                    }
                }
            }
        }
    }

    jvm_arguments.push(get_logging_argument(&minecraft_manifest, &base_dir));

    println!("JVM arguments successfully created");

    jvm_arguments
}

fn get_logging_argument (
    minecraft_manifest: &MinecraftManifest,
    base_dir: &Path,
) -> String {
    let log_config_path = base_dir
        .join("assets")
        .join("log-configs")
        .join(&minecraft_manifest.logging.client.file.id);

    let logging_argument = minecraft_manifest
        .logging
        .client
        .argument
        .replace("${path}", &log_config_path.to_string_lossy());

    logging_argument
}
