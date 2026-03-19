use futures::future::{join_all, BoxFuture};
use futures::FutureExt;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, redirect::Policy};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const BASE_URL: &str = "https://www.curseforge.com";

pub mod ffi;

fn defaultheaders(accept: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "User-Agent",
        HeaderValue::from_static(
            "Mozilla/5.0 (X11; Linux x86_64; rv:148.0) Gecko/20100101 Firefox/148.0",
        ),
    );
    headers.insert("Accept-Language", HeaderValue::from_static("en-US,en;q=0.9"));
    headers.insert("Sec-GPC", HeaderValue::from_static("1"));
    headers.insert("Upgrade-Insecure-Requests", HeaderValue::from_static("1"));
    headers.insert("Sec-Fetch-Dest", HeaderValue::from_static("document"));
    headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("navigate"));
    headers.insert("Sec-Fetch-Site", HeaderValue::from_static("none"));
    headers.insert("Sec-Fetch-User", HeaderValue::from_static("?1"));
    headers.insert("Priority", HeaderValue::from_static("u=0, i"));
    headers.insert("Accept", HeaderValue::from_str(accept).unwrap_or(HeaderValue::from_static("*/*")));
    headers
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct dependencyinfo {
    pub name: String,
    pub author: String,
    pub dllink: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct modinfo {
    pub name: String,
    pub author: String,
    pub description: String,
    pub downloads: String,
    pub updated: String,
    pub gameversion: String,
    pub mainmodloader: String,
    pub dllink: String,
    pub dependencies: Vec<dependencyinfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct fileinfo {
    pub filename: String,
    pub versions: Vec<String>,
    pub loaders: Vec<String>,
    pub uploaded: String,
    pub size: String,
    pub downloads: String,
    pub fileurl: String,
    pub jardlurl: String,
}

fn cap(re: &Regex, text: &str, idx: usize) -> String {
    re.captures(text)
        .and_then(|c| c.get(idx))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

fn striptags(input: &str) -> String {
    let re = Regex::new(r"<[^>]+>").unwrap();
    re.replace_all(input, "").to_string()
}

pub async fn getmoddeps(modpath: &str, client: &Client) -> reqwest::Result<Vec<dependencyinfo>> {
    let url = format!("{BASE_URL}{modpath}/relations/dependencies");
    let data = client
        .get(url)
        .headers(defaultheaders("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"))
        .send()
        .await?
        .text()
        .await?;

    let cardre = Regex::new(r#"<a class=\"related-project-card\"(.*?)</a>"#).unwrap();
    let hrefre = Regex::new(r#"href=\"([^\"]+)\""#).unwrap();
    let namere = Regex::new(r#"<h5[^>]*>(.*?)</h5>"#).unwrap();
    let authorre = Regex::new(r#"class=\"author-name\"[^>]*><span[^>]*>(.*?)</span>"#).unwrap();

    let mut deps = Vec::new();
    for m in cardre.captures_iter(&data) {
        let block = m.get(1).map(|v| v.as_str()).unwrap_or("");
        let path = cap(&hrefre, block, 1);
        let name = cap(&namere, block, 1);
        let author = cap(&authorre, block, 1);
        deps.push(dependencyinfo {
            name: name.trim().to_string(),
            author: author.trim().to_string(),
            dllink: format!("{BASE_URL}{path}"),
        });
    }

    Ok(deps)
}

pub async fn getjarurl(
    fileurl: &str,
    filename: Option<&str>,
    client: &Client,
) -> reqwest::Result<Option<String>> {
    let fileidre = Regex::new(r"/files/(\d+)$").unwrap();
    let Some(fid_caps) = fileidre.captures(fileurl) else {
        return Ok(None);
    };
    let fileid = fid_caps.get(1).unwrap().as_str();

    if let Some(name) = filename {
        if name.to_lowercase().ends_with(".jar") {
            let part1 = &fileid[..fileid.len().min(4)];
            let part2 = &fileid[fileid.len().min(4)..];
            let encoded = urlencoding::encode(name);
            return Ok(Some(format!(
                "https://mediafilez.forgecdn.net/files/{part1}/{part2}/{encoded}"
            )));
        }
    }

    let data = client
        .get(fileurl)
        .headers(defaultheaders("*/*"))
        .send()
        .await?
        .text()
        .await?;

    let projre = Regex::new(r#"\\?\"id\\?\":(\d+),\\?\"gameId\\?\":\d+"#).unwrap();
    let Some(proj_caps) = projre.captures(&data) else {
        return Ok(None);
    };
    let projid = proj_caps.get(1).unwrap().as_str();

    let apiuri = format!("{BASE_URL}/api/v1/mods/{projid}/files/{fileid}/download");
    let noredirect = Client::builder().redirect(Policy::none()).build()?;
    let resp = noredirect
        .get(&apiuri)
        .headers(defaultheaders("*/*"))
        .send()
        .await?;
    if let Some(loc1) = resp.headers().get("Location") {
        let loc1 = loc1.to_str().unwrap_or(&apiuri).to_string();
        let resp2 = noredirect
            .get(&loc1)
            .headers(defaultheaders("*/*"))
            .send()
            .await?;
        if let Some(loc2) = resp2.headers().get("Location") {
            return Ok(Some(loc2.to_str().unwrap_or(&loc1).to_string()));
        }
        return Ok(Some(loc1));
    }

    Ok(Some(apiuri))
}

async fn parsefilerows(data: &str) -> Vec<fileinfo> {
    let rowre = Regex::new(r#"<a class=\"file-row-details\"(.*?)</a>"#).unwrap();
    let hrefre = Regex::new(r#"href=\"([^\"]+)\""#).unwrap();
    let namere = Regex::new(r#"class=\"name\"[^>]*title=\"([^\"]+)\""#).unwrap();
    let uploadedre = Regex::new(r#"<span><span>(.*?)</span></span>"#).unwrap();
    let sizere = Regex::new(r#"<span>(\d+\.?\d*\s*(?:KB|MB|GB))</span>"#).unwrap();
    let downloadsre = Regex::new(r#"class=\"ellipsis\">(.*?)</span>"#).unwrap();
    let versionsre = Regex::new(r#"<li>([\d.]+)</li>"#).unwrap();
    let loadersre = Regex::new(r#"<li>(Forge|Fabric|NeoForge|Quilt)</li>"#).unwrap();
    let loaderspanre = Regex::new(r#"class=\"detail-other detail-flavor\"[^>]*>(.*?)</div>"#).unwrap();
    let loaderfallbackre = Regex::new(r#"(Forge|Fabric|NeoForge|Quilt)"#).unwrap();

    let mut files = Vec::new();
    for m in rowre.captures_iter(data) {
        let row = m.get(1).map(|v| v.as_str()).unwrap_or("");
        let fpath = cap(&hrefre, row, 1);
        let filename = cap(&namere, row, 1);
        if filename.is_empty() {
            continue;
        }
        let uploaded = cap(&uploadedre, row, 1);
        let size = cap(&sizere, row, 1);
        let downloads = cap(&downloadsre, row, 1);
        let versions = versionsre
            .captures_iter(row)
            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
            .collect::<Vec<_>>();
        let mut loaders = loadersre
            .captures_iter(row)
            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
            .collect::<Vec<_>>();
        if loaders.is_empty() {
            if let Some(span) = loaderspanre.captures(row).and_then(|c| c.get(1)) {
                loaders = loaderfallbackre
                    .captures_iter(span.as_str())
                    .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                    .collect();
            }
        }

        files.push(fileinfo {
            filename: filename.trim().to_string(),
            versions,
            loaders,
            uploaded: uploaded.trim().to_string(),
            size: size.trim().to_string(),
            downloads: downloads.trim().to_string(),
            fileurl: format!("{BASE_URL}{fpath}"),
            jardlurl: String::new(),
        });
    }

    files
}

async fn getldrfiles(
    client: &Client,
    base: &str,
    page: usize,
    pagesize: usize,
) -> reqwest::Result<(Vec<fileinfo>, usize, HashMap<String, String>)> {
    let url = format!(
        "{base}/files/all?page={page}&pageSize={pagesize}&showAlphaFiles=hide"
    );
    let data = client
        .get(url)
        .headers(defaultheaders("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"))
        .send()
        .await?
        .text()
        .await?;

    let pagere = Regex::new(r#"<li class=\" \"?><button>(\d+)</button></li>"#).unwrap();
    let mut pages = page;
    for m in pagere.captures_iter(&data) {
        if let Some(n) = m.get(1).and_then(|v| v.as_str().parse::<usize>().ok()) {
            if n > pages {
                pages = n;
            }
        }
    }

    let mut fmap: HashMap<String, String> = HashMap::new();
    let mapre1 = Regex::new(r#"\\"id\\":(\d+),\\"fileName\\":\\"([^\\"]+)\\""#).unwrap();
    let mapre2 = Regex::new(r#"\"id\":(\d+),\"fileName\":\"([^\"]+)\""#).unwrap();
    for m in mapre1.captures_iter(&data) {
        let id = m.get(1).unwrap().as_str().to_string();
        let name = m.get(2).unwrap().as_str().to_string();
        if name.to_lowercase().ends_with(".jar") {
            fmap.insert(id, name);
        }
    }
    if fmap.is_empty() {
        for m in mapre2.captures_iter(&data) {
            let id = m.get(1).unwrap().as_str().to_string();
            let name = m.get(2).unwrap().as_str().to_string();
            if name.to_lowercase().ends_with(".jar") {
                fmap.insert(id, name);
            }
        }
    }

    let files = parsefilerows(&data).await;
    Ok((files, pages, fmap))
}

pub async fn getmodfiles(dllink: &str) -> reqwest::Result<Vec<fileinfo>> {
    let client = Client::builder().build()?;
    let base = dllink.trim_end_matches('/');
    let pagesize = 50usize;
    let (firstfiles, totalpages, mapfirst) = getldrfiles(&client, base, 1, pagesize).await?;

    let mut results = Vec::new();
    let mut mapall: HashMap<String, String> = mapfirst;
    results.extend(firstfiles);

    if totalpages > 1 {
        let mut tasks = Vec::new();
        for p in 2..=totalpages {
            tasks.push(getldrfiles(&client, base, p, pagesize));
        }
        let pages = join_all(tasks).await;
        for res in pages {
            if let Ok((files, _pages, fmap)) = res {
                results.extend(files);
                mapall.extend(fmap);
            }
        }
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut unique = Vec::new();
    for f in results {
        if seen.insert(f.fileurl.clone()) {
            unique.push(f);
        }
    }

    for f in unique.iter_mut() {
        if let Some(caps) = Regex::new(r"/files/(\d+)$").unwrap().captures(&f.fileurl) {
            let fid = caps.get(1).unwrap().as_str();
            if let Some(name) = mapall.get(fid) {
                f.filename = name.clone();
                let encoded = urlencoding::encode(name);
                f.jardlurl = format!(
                    "https://mediafilez.forgecdn.net/files/{}/{}/{}",
                    &fid[..fid.len().min(4)],
                    &fid[fid.len().min(4)..],
                    encoded
                );
                continue;
            }
        }
        if f.jardlurl.is_empty() {
            let url = getjarurl(&f.fileurl, Some(&f.filename), &client).await?;
            f.jardlurl = url.unwrap_or_default();
        }
    }

    Ok(unique)
}

pub async fn getmodslist(query: &str) -> reqwest::Result<Vec<modinfo>> {
    let client = Client::builder().build()?;
    let data = client
        .get("https://www.curseforge.com/minecraft/search")
        .headers(defaultheaders("*/*"))
        .query(&[
            ("page", "1"),
            ("pageSize", "9999"),
            ("sortBy", "relevancy"),
            ("search", query),
        ])
        .send()
        .await?
        .text()
        .await?;

    let cardre = Regex::new(r#"<div class=\" project-card\">(.*?)</div>\s*<div class=\" project-card\">"#).unwrap();
    let namere = Regex::new(r#"class=\"name\"[^>]*><span[^>]*>(.*?)</span>"#).unwrap();
    let authorre = Regex::new(r#"class=\"author-name\"[^>]*><span[^>]*>(.*?)</span>"#).unwrap();
    let descre = Regex::new(r#"class=\"description\">(.*?)</p>"#).unwrap();
    let downloadsre = Regex::new(r#"class=\"detail-downloads\">(.*?)</li>"#).unwrap();
    let updatedre = Regex::new(r#"class=\"detail-updated\"><span[^>]*>(.*?)</span>"#).unwrap();
    let gameverre = Regex::new(r#"class=\"detail-game-version\">(.*?)</li>"#).unwrap();
    let loaderre = Regex::new(r#"class=\"detail-flavor\">(.*?)</li>"#).unwrap();
    let uripathre = Regex::new(r#"class=\"overlay-link\"[^>]*href=\"([^\"]+)\""#).unwrap();

    let mut pcards = Vec::new();
    let mut padded = data.clone();
    padded.push_str("<div class=\" project-card\">");
    for card in cardre.captures_iter(&padded) {
        let block = card.get(1).map(|v| v.as_str()).unwrap_or("");
        let name = cap(&namere, block, 1);
        let author = cap(&authorre, block, 1);
        let description = cap(&descre, block, 1);
        let downloads = cap(&downloadsre, block, 1);
        let updated = cap(&updatedre, block, 1);
        let gamever = cap(&gameverre, block, 1);
        let mainmodloader = cap(&loaderre, block, 1);
        let uripath = cap(&uripathre, block, 1);
        pcards.push((
            name,
            author,
            description,
            downloads,
            updated,
            gamever,
            mainmodloader,
            uripath,
        ));
    }

    let mut deptasks: Vec<BoxFuture<'_, reqwest::Result<Vec<dependencyinfo>>>> = Vec::new();
    for (_, _, _, _, _, _, _, path) in pcards.iter() {
        if path.is_empty() {
            deptasks.push(async { Ok(Vec::new()) }.boxed());
        } else {
            let clientref = &client;
            let path = path.clone();
            deptasks.push(async move { getmoddeps(&path, clientref).await }.boxed());
        }
    }

    let deps = join_all(deptasks).await;

    let mut mods = Vec::new();
    for ((name, author, description, downloads, updated, gamever, mainmodloader, uripath), deps) in
        pcards.into_iter().zip(deps)
    {
        let deps = deps.unwrap_or_default();
        mods.push(modinfo {
            name: name.trim().to_string(),
            author: author.trim().to_string(),
            description: description.trim().to_string(),
            downloads: downloads.trim().to_string(),
            updated: updated.trim().to_string(),
            gameversion: gamever.trim().to_string(),
            mainmodloader: striptags(&mainmodloader).trim().to_string(),
            dllink: format!("{BASE_URL}{uripath}"),
            dependencies: deps,
        });
    }

    Ok(mods)
}

pub async fn getmodslistjson(query: &str) -> reqwest::Result<String> {
    let mods = getmodslist(query).await?;
    Ok(serde_json::to_string_pretty(&mods).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn teststriptags() {
        assert_eq!(striptags("<b>Forge</b>"), "Forge");
    }
}
