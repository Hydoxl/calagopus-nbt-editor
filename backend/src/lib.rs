use shared::{
    State,
    extensions::{
        Extension, ExtensionPermissionsBuilder, ExtensionRouteBuilder, ExtensionUpdateInfo,
    },
};

mod routes;
mod services;

const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/Hydoxl/calagopus-nbt-editor/releases/latest";

#[derive(Default)]
pub struct ExtensionStruct;

#[derive(serde::Deserialize, serde::Serialize)]
struct LatestRelease {
    tag_name: String,
    body: Option<String>,
}

impl LatestRelease {
    fn into_update_info(
        self,
        current_version: &semver::Version,
    ) -> Result<Option<ExtensionUpdateInfo>, anyhow::Error> {
        let version =
            semver::Version::parse(self.tag_name.strip_prefix('v').unwrap_or(&self.tag_name))?;
        if version <= *current_version {
            return Ok(None);
        }

        Ok(Some(ExtensionUpdateInfo {
            version,
            changes: self
                .body
                .unwrap_or_default()
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(compact_str::CompactString::from)
                .collect(),
        }))
    }
}

#[async_trait::async_trait]
impl Extension for ExtensionStruct {
    async fn check_for_updates(
        &self,
        state: State,
        current_version: &semver::Version,
    ) -> Result<Option<ExtensionUpdateInfo>, anyhow::Error> {
        let release: LatestRelease = state
            .cache
            .cached("xyz.nm15.nbteditor::latest-release", 3600, || async {
                state
                    .client
                    .get(LATEST_RELEASE_URL)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await
            })
            .await?;

        release.into_update_info(current_version)
    }

    async fn initialize(&mut self, _state: State) {
        tracing::info!("nbt editor extension initialized");
    }

    async fn initialize_router(
        &mut self,
        state: State,
        builder: ExtensionRouteBuilder,
    ) -> ExtensionRouteBuilder {
        builder.add_client_server_api_router(|router| {
            router.merge(routes::client_server_router(&state))
        })
    }

    async fn initialize_permissions(
        &mut self,
        _state: State,
        builder: ExtensionPermissionsBuilder,
    ) -> ExtensionPermissionsBuilder {
        builder
    }
}

#[cfg(test)]
mod tests {
    use super::LatestRelease;

    #[test]
    fn reports_newer_release_with_notes() {
        let release: LatestRelease = serde_json::from_str(
            r#"{"tag_name":"v1.0.10","body":"First change\n\nSecond change"}"#,
        )
        .unwrap();
        let update = release
            .into_update_info(&semver::Version::new(1, 0, 4))
            .unwrap()
            .unwrap();

        assert_eq!(update.version, semver::Version::new(1, 0, 10));
        assert_eq!(update.changes, vec!["First change", "Second change"]);
    }

    #[test]
    fn ignores_equal_and_older_releases() {
        for current in [semver::Version::new(1, 0, 4), semver::Version::new(1, 0, 5)] {
            let release = LatestRelease {
                tag_name: "v1.0.4".into(),
                body: None,
            };
            assert!(release.into_update_info(&current).unwrap().is_none());
        }
    }

    #[test]
    fn accepts_release_without_notes() {
        let release: LatestRelease =
            serde_json::from_str(r#"{"tag_name":"1.0.5","body":null}"#).unwrap();
        let update = release
            .into_update_info(&semver::Version::new(1, 0, 4))
            .unwrap()
            .unwrap();
        assert!(update.changes.is_empty());
    }

    #[test]
    fn rejects_invalid_release_version() {
        let release = LatestRelease {
            tag_name: "invalid".into(),
            body: None,
        };
        assert!(
            release
                .into_update_info(&semver::Version::new(1, 0, 4))
                .is_err()
        );
    }
}
