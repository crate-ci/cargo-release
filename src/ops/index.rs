use crate::config::CertsSource;
use cargo_utils::{registry_token, registry_url};
use secrecy::{ExposeSecret, SecretString};
use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::path::Path;
use tame_index::IndexUrl;
use tame_index::krate::IndexKrate;
use tame_index::utils::flock::FileLock;

/// Provides access to the remote registries (crates.io and custom ones)
#[derive(Default)]
pub struct CratesIndex {
    registries: std::collections::HashMap<Option<String>, RegistryIndex>,
}

impl CratesIndex {
    #[inline]
    pub fn new() -> Self {
        Self {
            registries: std::collections::HashMap::new(),
        }
    }

    fn resolve_registry(
        &mut self,
        manifest: &Path,
        registry: Option<&str>,
        certs_source: CertsSource,
    ) -> Result<&mut RegistryIndex, crate::error::CliError> {
        let registry_owned = registry.map(String::from);
        let entry = self.registries.entry(registry_owned);
        match entry {
            Entry::Occupied(registry) => Ok(registry.into_mut()),
            Entry::Vacant(entry) => {
                let (index_url, token) = if let Some(registry) = registry {
                    let url = registry_url(manifest, Some(registry))?.to_string();
                    let token = registry_token(Some(registry))?;
                    (IndexUrl::NonCratesIo(Cow::Owned(url)), token)
                } else {
                    (IndexUrl::CratesIoSparse, None)
                };

                let index = RegistryIndex::new(
                    registry.unwrap_or("crates-io").to_string(),
                    RemoteIndex::open(index_url, certs_source, token)?,
                );
                Ok(entry.insert(index))
            }
        }
    }

    /// Determines if the specified crate exists in the given registry
    #[inline]
    pub fn has_krate(
        &mut self,
        manifest: &Path,
        registry: Option<&str>,
        name: &str,
        certs_source: CertsSource,
    ) -> Result<bool, crate::error::CliError> {
        self.resolve_registry(manifest, registry, certs_source)?
            .has_krate(name)
    }

    /// Determines if the specified crate version exists in the given registry
    #[inline]
    pub fn has_krate_version(
        &mut self,
        manifest: &Path,
        registry: Option<&str>,
        name: &str,
        version: &str,
        certs_source: CertsSource,
    ) -> Result<Option<bool>, crate::error::CliError> {
        self.resolve_registry(manifest, registry, certs_source)?
            .has_krate_version(name, version)
    }
}

/// Provides access to a single remote registry (crates.io or custom)
pub struct RegistryIndex {
    registry_name: String,
    index: RemoteIndex,
    cache: std::collections::HashMap<String, Option<IndexKrate>>,
}

impl RegistryIndex {
    #[inline]
    pub fn new(registry_name: String, index: RemoteIndex) -> Self {
        Self {
            registry_name,
            index,
            cache: std::collections::HashMap::new(),
        }
    }

    /// Determines if the specified crate exists in this index
    #[inline]
    pub fn has_krate(&mut self, name: &str) -> Result<bool, crate::error::CliError> {
        Ok(self.krate(name)?.map(|_| true).unwrap_or(false))
    }

    /// Determines if the specified crate version exists in the crates.io index
    #[inline]
    pub fn has_krate_version(
        &mut self,
        name: &str,
        version: &str,
    ) -> Result<Option<bool>, crate::error::CliError> {
        let krate = self.krate(name)?;
        Ok(krate.map(|ik| ik.versions.iter().any(|iv| iv.version == version)))
    }

    pub(crate) fn krate(
        &mut self,
        krate_name: &str,
    ) -> Result<Option<IndexKrate>, crate::error::CliError> {
        if let Some(entry) = self.cache.get(krate_name) {
            log::trace!(
                "Reusing index for {krate_name} from registry {}",
                self.registry_name
            );
            return Ok(entry.clone());
        }

        log::trace!(
            "Downloading index for {krate_name} from registry {}",
            self.registry_name
        );
        let entry = self.index.krate(krate_name)?;
        self.cache.insert(krate_name.to_owned(), entry.clone());
        Ok(entry)
    }
}

pub struct RemoteIndex {
    index: tame_index::SparseIndex,
    client: tame_index::external::reqwest::blocking::Client,
    lock: FileLock,
    etags: Vec<(String, String)>,
}

impl RemoteIndex {
    #[inline]
    pub fn open(
        index_url: IndexUrl<'_>,
        certs_source: CertsSource,
        token: Option<SecretString>,
    ) -> Result<Self, crate::error::CliError> {
        let index = tame_index::SparseIndex::new(tame_index::IndexLocation::new(index_url))?;
        let client = {
            let builder = tame_index::external::reqwest::blocking::ClientBuilder::new();

            let builder = match certs_source {
                CertsSource::Webpki => builder.tls_built_in_webpki_certs(true),
                CertsSource::Native => builder.tls_built_in_native_certs(true),
            };

            let builder = match token {
                None => builder,
                Some(token) => {
                    let mut headers = tame_index::external::reqwest::header::HeaderMap::new();
                    let token = token.expose_secret();
                    let authorization_header =
                        tame_index::external::http::header::HeaderValue::from_str(token).map_err(
                            |e| anyhow::anyhow!("Failed to set authorization header {:?}", e),
                        )?;
                    headers.insert(
                        tame_index::external::http::header::AUTHORIZATION,
                        authorization_header,
                    );
                    builder.default_headers(headers)
                }
            };

            builder.build()?
        };

        let lock = FileLock::unlocked();

        Ok(Self {
            index,
            client,
            lock,
            etags: Vec::new(),
        })
    }

    pub(crate) fn krate(
        &mut self,
        name: &str,
    ) -> Result<Option<IndexKrate>, crate::error::CliError> {
        let etag = self
            .etags
            .iter()
            .find_map(|(krate, etag)| (krate == name).then_some(etag.as_str()))
            .unwrap_or("");

        let krate_name = name.try_into()?;
        let req = self
            .index
            .make_remote_request(krate_name, Some(etag), &self.lock)?;
        let (
            tame_index::external::http::request::Parts {
                method,
                uri,
                version,
                headers,
                ..
            },
            _,
        ) = req.into_parts();
        let mut req = self.client.request(method, uri.to_string());
        req = req.version(version);
        req = req.headers(headers);
        let res = self.client.execute(req.build()?)?;

        // Grab the etag if it exists for future requests
        if let Some(etag) = res
            .headers()
            .get(tame_index::external::reqwest::header::ETAG)
            && let Ok(etag) = etag.to_str()
        {
            if let Some(i) = self.etags.iter().position(|(krate, _)| krate == name) {
                etag.clone_into(&mut self.etags[i].1);
            } else {
                self.etags.push((name.to_owned(), etag.to_owned()));
            }
        }

        let mut builder = tame_index::external::http::Response::builder()
            .status(res.status())
            .version(res.version());

        builder
            .headers_mut()
            .unwrap()
            .extend(res.headers().iter().map(|(k, v)| (k.clone(), v.clone())));

        let body = res.bytes()?;
        let response = builder
            .body(body.to_vec())
            .map_err(|e| tame_index::Error::from(tame_index::error::HttpError::from(e)))?;

        self.index
            .parse_remote_response(krate_name, response, false, &self.lock)
            .map_err(Into::into)
    }
}
