use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::{collections::HashMap, path::Path};

use futures::stream::Filter;
use futures::{stream, StreamExt}; // 0.3.1
use std::error::Error;

use steam_shortcuts_util::shortcut::ShortcutOwned;
use steamgriddb_api::Client;

use crate::settings::Settings;
use crate::steam::{get_shortcuts_for_user, get_users_images, SteamUsersInfo};
use crate::steamgriddb::ImageType;

use super::CachedSearch;

const CONCURRENT_REQUESTS: usize = 10;

pub async fn download_images_for_users<'b>(settings: &Settings, users: &Vec<SteamUsersInfo>) {
    let start_time = std::time::Instant::now();

    let to_downloads = stream::iter(users)
        .map(|user| {
            let shortcut_info = get_shortcuts_for_user(user);
            async move {
                start_search_for_to_download(settings, user, &shortcut_info.shortcuts)
                    .await
                    .unwrap_or(vec![])
            }
        })
        .buffer_unordered(CONCURRENT_REQUESTS)
        .collect::<Vec<Vec<ToDownload>>>()
        .await;
    let to_downloads = to_downloads.iter().flatten().collect::<Vec<&ToDownload>>();

    stream::iter(to_downloads)
        .map(|to_download| async move {
            if let Err(e) = download_to_download(&to_download).await {
                println!("Error downloading {:?}: {}", &to_download.path, e);
            }
        })
        .buffer_unordered(CONCURRENT_REQUESTS)
        .collect::<Vec<()>>()
        .await;
    let duration = start_time.elapsed();

    println!("Finished getting images in: {:?}", duration);
}

async fn start_search_for_to_download(
    settings: &Settings,
    user: &crate::steam::SteamUsersInfo,
    shortcut_info: &Vec<ShortcutOwned>,
) -> Result<Vec<ToDownload>, Box<dyn Error>> {
    let auth_key = &settings.steamgrid_db.auth_key;

    if let Some(auth_key) = auth_key {
        println!("Checking for game images");
        let client = steamgriddb_api::Client::new(auth_key);
        let mut search = CachedSearch::new(&client);
        let known_images = get_users_images(user).unwrap();
        let res = search_fo_to_download(
            known_images,
            user.steam_user_data_folder.as_str(),
            shortcut_info,
            &mut search,
            &client,
        )
        .await?;
        search.save();
        Ok(res)
    } else {
        println!("Steamgrid DB Auth Key not found, please add one as described here:  https://github.com/PhilipK/steam_shortcuts_sync#configuration");
        Ok(Vec::new())
    }
}

async fn search_fo_to_download<'b>(
    known_images: Vec<String>,
    user_data_folder: &str,
    shortcuts: &Vec<ShortcutOwned>,
    search: &mut CachedSearch<'b>,
    client: &Client,
) -> Result<Vec<ToDownload>, Box<dyn Error>> {
    let shortcuts_to_search_for = shortcuts.iter().filter(|s| {
        let images = vec![
            format!("{}_hero.png", s.app_id),
            format!("{}p.png", s.app_id),
            format!("{}_logo.png", s.app_id),
        ];
        // if we are missing any of the images we need to search for them
        images.iter().any(|image| !known_images.contains(&image)) && "" != s.app_name
    });
    if shortcuts_to_search_for.clone().count() == 0 {
        return Ok(vec![]);
    }
    let mut search_results = HashMap::new();
    let search_results_a = stream::iter(shortcuts_to_search_for)
        .map(|s| async move {
            let search_result = search.search(s.app_id, &s.app_name).await;
            if search_result.is_err() {
                return None;
            }
            let search_result = search_result.unwrap();
            if search_result.is_none() {
                return None;
            }
            let search_result = search_result.unwrap();
            Some((s.app_id, search_result))
        })
        .buffer_unordered(CONCURRENT_REQUESTS)
        .collect::<Vec<Option<(u32, usize)>>>()
        .await;
    for r in search_results_a {
        if let Some((app_id, search)) = r {
            search_results.insert(app_id, search);
        }
    }
    let types = vec![ImageType::Logo, ImageType::Hero, ImageType::Grid];
    let mut to_download = vec![];
    for image_type in types {
        let mut images_needed = shortcuts
            .iter()
            .filter(|s| search_results.contains_key(&s.app_id))
            .filter(|s| !known_images.contains(&image_type.file_name(s.app_id)));
        let image_ids: Vec<usize> = images_needed
            .clone()
            .filter_map(|s| search_results.get(&s.app_id))
            .map(|search| *search)
            .collect();
        use steamgriddb_api::query_parameters::QueryType::*;
        let query_type = match image_type {
            ImageType::Hero => Hero(None),
            ImageType::Grid => Grid(None),
            ImageType::Logo => Logo(None),
        };

        match client
            .get_images_for_ids(image_ids.as_slice(), &query_type)
            .await
        {
            Ok(images) => {
                for image in images {
                    if let Some(shortcut) = images_needed.next() {
                        if let Ok(image) = image {
                            let grid_folder = Path::new(user_data_folder).join("config/grid");
                            let path = grid_folder.join(image_type.file_name(shortcut.app_id));
                            to_download.push(ToDownload {
                                path,
                                url: image.url,
                            });
                        }
                    }
                }
            }
            Err(err) => println!("Error getting images: {}", err),
        }
    }
    Ok(to_download)
}

async fn download_to_download(to_download: &ToDownload) -> Result<(), Box<dyn Error>> {
    println!("Downloading {} to {:?}", to_download.url, to_download.path);
    let path = &to_download.path;
    let url = &to_download.url;
    let mut file = File::create(path).unwrap();
    let response = reqwest::get(url).await?;
    let content = response.bytes().await?;
    file.write_all(&content).unwrap();
    Ok(())
}

pub struct ToDownload {
    path: PathBuf,
    url: String,
}
