pub mod dependencies;
pub mod target;

use crate::terminal::emoji;

use binary_install::{Cache, Download};
use log::info;
use semver::Version;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use lazy_static::lazy_static;

lazy_static! {
    static ref CACHE: Cache = get_wrangler_cache().expect("Could not get Wrangler cache location");
}

enum ToolDownload {
    NeedsInstall(Version),
    InstalledAt(Download),
}

pub fn install_cargo_generate() -> Result<PathBuf, failure::Error> {
    let tool_name = "cargo-generate";
    let tool_author = "ashleygwilliams";
    let is_binary = true;
    let version = Version::parse(dependencies::GENERATE_VERSION)?;
    install(tool_name, tool_author, is_binary, version)?.binary(tool_name)
}

pub fn install_wasm_pack() -> Result<PathBuf, failure::Error> {
    let tool_name = "wasm-pack";
    let tool_author = "rustwasm";
    let is_binary = true;
    let version = Version::parse(dependencies::WASM_PACK_VERSION)?;
    install(tool_name, tool_author, is_binary, version)?.binary(tool_name)
}

pub fn install(
    tool_name: &str,
    owner: &str,
    is_binary: bool,
    version: Version,
) -> Result<Download, failure::Error> {
    let download = match tool_needs_update(tool_name, version)? {
        ToolDownload::NeedsInstall(version) => {
            println!("{}  Installing {} v{}...", emoji::DOWN, tool_name, version);
            let binaries: Vec<&str> = if is_binary { vec![tool_name] } else { vec![] };
            let download =
                download_prebuilt(tool_name, owner, &version.to_string(), binaries.as_ref());
            match download {
                Ok(download) => Ok(download),
                Err(e) => Err(failure::format_err!(
                    "could not download `{}`\n{}",
                    tool_name,
                    e
                )),
            }
        }
        ToolDownload::InstalledAt(download) => Ok(download),
    }?;
    log::debug!("tool {} located at {:?}", tool_name, download);
    Ok(download)
}

fn tool_needs_update(
    tool_name: &str,
    target_version: Version,
) -> Result<ToolDownload, failure::Error> {
    let current_installation = get_installation(tool_name, &target_version);
    // if something goes wrong checking the current installation
    // we shouldn't fail, we should just re-install for them
    if let Ok(current_installation) = current_installation {
        if let Some((installed_version, installed_location)) = current_installation {
            if installed_version.major == target_version.major
                && installed_version >= target_version
            {
                return Ok(ToolDownload::InstalledAt(Download::at(&installed_location)));
            }
        }
    }
    Ok(ToolDownload::NeedsInstall(target_version))
}

fn get_installation(
    tool_name: &str,
    target_version: &Version,
) -> Result<Option<(Version, PathBuf)>, failure::Error> {
    for entry in fs::read_dir(&CACHE.destination)? {
        let entry = entry?;
        let filename = entry.file_name().into_string();
        if let Ok(filename) = filename {
            if filename.starts_with(tool_name) {
                let installed_version = filename
                    .split(&format!("{}-", tool_name))
                    .collect::<Vec<&str>>()[1];
                let installed_version = Version::parse(installed_version);
                // if the installed version can't be parsed, ignore it
                if let Ok(installed_version) = installed_version {
                    if &installed_version == target_version {
                        return Ok(Some((installed_version, entry.path())));
                    }
                }
            }
        }
    }
    Ok(None)
}

fn download_prebuilt(
    tool_name: &str,
    owner: &str,
    version: &str,
    binaries: &[&str],
) -> Result<Download, failure::Error> {
    let url = match prebuilt_url(tool_name, owner, version) {
        Some(url) => url,
        None => failure::bail!(format!(
            "no prebuilt {} binaries are available for this platform",
            tool_name
        )),
    };

    info!("prebuilt artifact {}", url);

    // no binaries are expected; downloading it as an artifact
    let res = if !binaries.is_empty() {
        CACHE.download_version(true, tool_name, binaries, &url, version)?
    } else {
        CACHE.download_artifact_version(tool_name, &url, version)?
    };

    match res {
        Some(download) => Ok(download),
        None => failure::bail!("{} is not installed!", tool_name),
    }
}

fn prebuilt_url(tool_name: &str, owner: &str, version: &str) -> Option<String> {
    if tool_name == "wranglerjs" {
        Some(format!(
            "https://workers.cloudflare.com/get-wranglerjs-binary/{0}/v{1}.tar.gz",
            tool_name, version
        ))
    } else {
        let target = if target::LINUX && target::x86_64 {
            "x86_64-unknown-linux-musl"
        } else if target::MACOS && target::x86_64 {
            "x86_64-apple-darwin"
        } else if target::WINDOWS && target::x86_64 {
            "x86_64-pc-windows-msvc"
        } else if target::MACOS && target::aarch64 {
            "aarch64-apple-darwin"
        } else {
            return None;
        };

        let url = if target == "aarch64-apple-darwin" {            
            let override_url = format!(
                "https://workers.cloudflare.com/get-override/{0}/{1}/v{2}/{3}.tar.gz",
                owner, tool_name, version, target
            );
            println!("sent to override URL: {}", override_url);
            override_url
        } else {
            format!(
                "https://workers.cloudflare.com/get-binary/{0}/{1}/v{2}/{3}.tar.gz",
                owner, tool_name, version, target
            )
        };
        Some(url)
    }
}

fn get_wrangler_cache() -> Result<Cache, failure::Error> {
    if let Ok(path) = env::var("WRANGLER_CACHE") {
        Ok(Cache::at(Path::new(&path)))
    } else {
        Cache::new("wrangler")
    }
}
