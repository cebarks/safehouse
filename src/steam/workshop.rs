use anyhow::{Context, Result};
use serde_json::Value;

const DETAILS_URL: &str =
    "https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/";

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
}
