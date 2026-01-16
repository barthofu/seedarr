use serde::Deserialize;

use crate::utils::Error;

#[derive(Debug, Clone)]
pub struct SonarrClient {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
}

impl SonarrClient {
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            client: reqwest::Client::new(),
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("{}/api/v3/{}", self.base_url, path.trim_start_matches('/'))
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, Error> {
        let url = self.api_url(path);
        let mut req = self
            .client
            .get(url)
            .header("Accept", "application/json")
            .header("X-Api-Key", &self.api_key);

        if !query.is_empty() {
            req = req.query(query);
        }

        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Other(format!(
                "Sonarr API request failed: HTTP {status} body={body}"
            )));
        }

        Ok(resp.json::<T>().await?)
    }

    pub async fn list_series(&self) -> Result<Vec<SeriesResource>, Error> {
        self.get_json("series", &[]).await
    }

    pub async fn list_episodes(&self, series_id: i64) -> Result<Vec<EpisodeResource>, Error> {
        self.get_json("episode", &[("seriesId", series_id.to_string())])
            .await
    }

    pub async fn list_episode_files(
        &self,
        series_id: i64,
    ) -> Result<Vec<EpisodeFileResource>, Error> {
        self.get_json("episodefile", &[("seriesId", series_id.to_string())])
            .await
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesResource {
    pub id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub series_type: Option<String>,
    pub overview: Option<String>,
    #[serde(default)]
    pub images: Vec<ImageResource>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageResource {
    pub remote_url: Option<String>,
    pub url: Option<String>,
    pub cover_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeResource {
    pub id: i64,
    pub season_number: i32,
    pub episode_number: i32,
    pub absolute_episode_number: Option<i32>,
    pub title: Option<String>,
    pub overview: Option<String>,
    #[serde(default)]
    pub has_file: bool,
    #[serde(default)]
    pub monitored: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeFileResource {
    pub id: i64,
    pub path: String,
    #[serde(default)]
    pub season_number: Option<i32>,
    pub scene_name: Option<String>,
    pub release_group: Option<String>,
    #[serde(default)]
    pub episode_ids: Vec<i64>,
    pub quality: Option<QualityModel>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityModel {
    pub quality: Option<QualityName>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityName {
    pub name: Option<String>,
}
