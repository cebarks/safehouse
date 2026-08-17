use anyhow::{Context, Result};
use serde_json::Value;

const DETAILS_URL: &str =
    "https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/";

const COLLECTION_URL: &str =
    "https://api.steampowered.com/ISteamRemoteStorage/GetCollectionDetails/v1/";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkshopModInfo {
    pub workshop_id: String,
    pub title: String,
    pub author: Option<String>,
    pub description: Option<String>,
}

pub fn parse_file_details(detail: &Value) -> Option<WorkshopModInfo> {
    Some(WorkshopModInfo {
        workshop_id: detail["publishedfileid"].as_str()?.to_owned(),
        title: detail["title"].as_str().unwrap_or("Unknown").to_owned(),
        author: detail["creator"].as_str().map(str::to_owned),
        description: detail["description"].as_str().map(str::to_owned),
    })
}

/// Fetch metadata for a single Workshop item.
pub async fn fetch_mod_info(
    client: &reqwest::Client,
    workshop_id: &str,
) -> Result<WorkshopModInfo> {
    let params = [("itemcount", "1"), ("publishedfileids[0]", workshop_id)];
    let resp: Value = client
        .post(DETAILS_URL)
        .form(&params)
        .send()
        .await
        .context("Steam API request failed")?
        .json()
        .await
        .context("Steam API response parse failed")?;

    let detail = &resp["response"]["publishedfiledetails"][0];
    parse_file_details(detail)
        .with_context(|| format!("no details returned for workshop ID {workshop_id}"))
}

/// Fetch metadata for multiple Workshop items in one API call.
pub async fn fetch_mod_info_batch(
    client: &reqwest::Client,
    workshop_ids: &[String],
) -> Result<Vec<WorkshopModInfo>> {
    if workshop_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut params: Vec<(String, String)> =
        vec![("itemcount".to_string(), workshop_ids.len().to_string())];
    for (i, id) in workshop_ids.iter().enumerate() {
        params.push((format!("publishedfileids[{i}]"), id.clone()));
    }
    let resp: Value = client
        .post(DETAILS_URL)
        .form(&params)
        .send()
        .await
        .context("Steam API batch request failed")?
        .json()
        .await
        .context("Steam API batch response parse failed")?;

    let details = resp["response"]["publishedfiledetails"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    Ok(details.iter().filter_map(parse_file_details).collect())
}

/// Fetch all workshop item IDs from a Steam Workshop collection.
pub async fn fetch_collection_items(
    client: &reqwest::Client,
    collection_id: &str,
) -> Result<Vec<String>> {
    let params = [
        ("collectioncount", "1"),
        ("publishedfileids[0]", collection_id),
    ];
    let resp: Value = client
        .post(COLLECTION_URL)
        .form(&params)
        .send()
        .await
        .context("Steam collection API request failed")?
        .json()
        .await
        .context("Steam collection API response parse failed")?;

    let details = &resp["response"]["collectiondetails"];
    let collection = details
        .as_array()
        .and_then(|arr| arr.first())
        .with_context(|| format!("no collection found for ID {collection_id}"))?;

    let result_code = collection["result"].as_i64().unwrap_or(0);
    if result_code != 1 {
        anyhow::bail!(
            "Steam API returned error for collection {collection_id} (result={result_code}). \
             Check that the collection exists and is public."
        );
    }

    let children = collection["children"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let ids: Vec<String> = children
        .iter()
        .filter_map(|c| c["publishedfileid"].as_str().map(str::to_owned))
        .collect();
    Ok(ids)
}

/// Parse a collection ID from a URL or raw numeric string.
/// Accepts:
///   - `3383526786` (raw ID)
///   - `https://steamcommunity.com/sharedfiles/filedetails/?id=3383526786`
///   - `https://steamcommunity.com/workshop/filedetails/?id=3383526786`
pub fn parse_collection_id(input: &str) -> Result<String> {
    let trimmed = input.trim();
    // Raw numeric ID
    if trimmed.chars().all(|c| c.is_ascii_digit()) && !trimmed.is_empty() {
        return Ok(trimmed.to_string());
    }
    // URL with ?id= parameter
    if let Some(pos) = trimmed.find("id=") {
        let after = &trimmed[pos + 3..];
        let id: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !id.is_empty() {
            return Ok(id);
        }
    }
    anyhow::bail!(
        "cannot parse collection ID from {trimmed:?}. \
         Provide a numeric ID or a Steam Workshop collection URL."
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_workshop_info_struct() {
        let info = WorkshopModInfo {
            workshop_id: "2392987220".to_string(),
            title: "Brita's Weapon Pack".to_string(),
            author: Some("Brita".to_string()),
            description: None,
        };
        assert_eq!(info.workshop_id, "2392987220");
    }

    #[test]
    fn test_parse_api_response() {
        let json = serde_json::json!({
            "response": {
                "publishedfiledetails": [{
                    "publishedfileid": "2392987220",
                    "title": "Brita's Weapon Pack",
                    "creator": "76561198XXXXX",
                    "description": "Adds weapons"
                }]
            }
        });
        let info =
            parse_file_details(&json["response"]["publishedfiledetails"][0]).unwrap();
        assert_eq!(info.title, "Brita's Weapon Pack");
        assert_eq!(info.workshop_id, "2392987220");
        assert_eq!(info.author.as_deref(), Some("76561198XXXXX"));
        assert_eq!(info.description.as_deref(), Some("Adds weapons"));
    }

    #[test]
    fn test_parse_missing_optional_fields() {
        let json = serde_json::json!({
            "publishedfileid": "12345",
            "title": "TestMod"
        });
        let info = parse_file_details(&json).unwrap();
        assert_eq!(info.workshop_id, "12345");
        assert_eq!(info.title, "TestMod");
        assert!(info.author.is_none());
        assert!(info.description.is_none());
    }

    #[test]
    fn test_parse_missing_publishedfileid_returns_none() {
        let json = serde_json::json!({
            "title": "TestMod"
        });
        assert!(parse_file_details(&json).is_none());
    }

    #[test]
    fn test_parse_collection_id_raw_numeric() {
        assert_eq!(parse_collection_id("3264403312").unwrap(), "3264403312");
    }

    #[test]
    fn test_parse_collection_id_with_whitespace() {
        assert_eq!(parse_collection_id("  3264403312  ").unwrap(), "3264403312");
    }

    #[test]
    fn test_parse_collection_id_from_sharedfiles_url() {
        let url = "https://steamcommunity.com/sharedfiles/filedetails/?id=3264403312";
        assert_eq!(parse_collection_id(url).unwrap(), "3264403312");
    }

    #[test]
    fn test_parse_collection_id_from_workshop_url() {
        let url = "https://steamcommunity.com/workshop/filedetails/?id=3264403312";
        assert_eq!(parse_collection_id(url).unwrap(), "3264403312");
    }

    #[test]
    fn test_parse_collection_id_url_with_extra_params() {
        let url = "https://steamcommunity.com/sharedfiles/filedetails/?id=3264403312&searchtext=";
        assert_eq!(parse_collection_id(url).unwrap(), "3264403312");
    }

    #[test]
    fn test_parse_collection_id_rejects_empty() {
        assert!(parse_collection_id("").is_err());
    }

    #[test]
    fn test_parse_collection_id_rejects_text() {
        assert!(parse_collection_id("not-a-number").is_err());
    }

    #[test]
    fn test_parse_collection_id_rejects_url_without_id() {
        assert!(parse_collection_id("https://steamcommunity.com/sharedfiles/").is_err());
    }
}
