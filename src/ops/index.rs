use crate::config::CertsSource;
use tame_index::krate::IndexKrate;
use tame_index::utils::flock::FileLock;

#[derive(Default)]
pub struct CratesIoIndex {
    index: Option<RemoteIndex>,
    cache: std::collections::HashMap<String, Option<IndexKrate>>,
}

impl CratesIoIndex {
    #[inline]
    pub fn new() -> Self {
        Self {
            index: None,
            cache: std::collections::HashMap::new(),
        }
    }

    /// Determines if the specified crate exists in the crates.io index
    #[inline]
    pub fn has_krate(
        &mut self,
        registry: Option<&str>,
        name: &str,
        certs_source: CertsSource,
    ) -> Result<bool, crate::error::CliError> {
        Ok(self
            .krate(registry, name, certs_source)?
            .map(|_| true)
            .unwrap_or(false))
    }

    /// Determines if the specified crate version exists in the crates.io index
    #[inline]
    pub fn has_krate_version(
        &mut self,
        registry: Option<&str>,
        name: &str,
        version: &str,
        certs_source: CertsSource,
    ) -> Result<Option<bool>, crate::error::CliError> {
        let krate = self.krate(registry, name, certs_source)?;
        Ok(krate.map(|ik| ik.versions.iter().any(|iv| iv.version == version)))
    }

    #[inline]
    pub fn update_krate(&mut self, registry: Option<&str>, name: &str) {
        if registry.is_some() {
            return;
        }

        self.cache.remove(name);
    }

    pub(crate) fn krate(
        &mut self,
        registry: Option<&str>,
        name: &str,
        certs_source: CertsSource,
    ) -> Result<Option<IndexKrate>, crate::error::CliError> {
        if let Some(registry) = registry {
            log::trace!("Cannot connect to registry `{registry}`");
            return Ok(None);
        }

        if let Some(entry) = self.cache.get(name) {
            log::trace!("Reusing index for {name}");
            return Ok(entry.clone());
        }

        if self.index.is_none() {
            log::trace!("Connecting to index");
            self.index = Some(RemoteIndex::open(certs_source)?);
        }
        let index = self.index.as_mut().unwrap();
        log::trace!("Downloading index for {name}");
        let entry = index.krate(name)?;
        self.cache.insert(name.to_owned(), entry.clone());
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
    pub fn open(certs_source: CertsSource) -> Result<Self, crate::error::CliError> {
        let index = tame_index::SparseIndex::new(tame_index::IndexLocation::new(
            tame_index::IndexUrl::CratesIoSparse,
        ))?;

        let client = {
            let builder = tame_index::external::reqwest::blocking::ClientBuilder::new();

            let builder = match certs_source {
                CertsSource::Webpki => builder.tls_built_in_webpki_certs(true),
                CertsSource::Native => builder.tls_built_in_native_certs(true),
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
