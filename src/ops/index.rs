use crate::config::CertsSource;
use semver::Version;
use tame_index::flock::FileLock;

type VersionSet = Vec<Version>;

#[derive(Default)]
pub struct CratesIoIndex {
    index: Option<RemoteIndex>,
    cache: std::collections::HashMap<String, Option<VersionSet>>,
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
        let version = version.parse::<Version>()?;
        Ok(krate.map(|ik| ik.iter().any(|iv| iv == &version)))
    }

    #[inline]
    pub fn has_krate_version_req(
        &mut self,
        registry: Option<&str>,
        name: &str,
        version_req: &semver::VersionReq,
        certs_source: CertsSource,
    ) -> Result<Option<bool>, crate::error::CliError> {
        let krate = self.krate(registry, name, certs_source)?;
        Ok(krate.map(|ik| ik.iter().any(|iv| version_req.matches(iv))))
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
    ) -> Result<Option<VersionSet>, crate::error::CliError> {
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
    client: ureq::Agent,
    lock: FileLock,
    etag_cache: tame_index::EtagCache,
}

impl RemoteIndex {
    #[inline]
    pub fn open(certs_source: CertsSource) -> Result<Self, crate::error::CliError> {
        let index = tame_index::SparseIndex::new(tame_index::IndexLocation::new(
            tame_index::IndexUrl::CratesIoSparse,
        ))?;

        let client = ureq::Agent::config_builder()
            // tame_index handles HTTP status codes internally
            .http_status_as_error(false)
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .provider(ureq::tls::TlsProvider::Rustls)
                    .root_certs(match certs_source {
                        CertsSource::Webpki => ureq::tls::RootCerts::WebPki,
                        CertsSource::Native => ureq::tls::RootCerts::PlatformVerifier,
                    })
                    .build(),
            )
            .build()
            .new_agent();

        let lock = FileLock::unlocked();

        Ok(Self {
            index,
            client,
            lock,
            etag_cache: Default::default(),
        })
    }

    pub(crate) fn krate(
        &mut self,
        name: &str,
    ) -> Result<Option<VersionSet>, crate::error::CliError> {
        let krate_name = name.try_into()?;
        let req = self
            .etag_cache
            .make_remote_request(&self.index, krate_name, &self.lock)?;

        let (parts, mut body) = self.client.run(req.into())?.into_parts();
        let body = body.read_to_vec()?;

        self.etag_cache
            .parse_remote_response(
                &self.index,
                krate_name,
                &ureq::http::Response::from_parts(parts, body),
                false,
                &self.lock,
            )
            .and_then(|ie| {
                let Some(ie) = ie else {
                    return Ok(None);
                };

                ie.iter()?
                    .map(|entry| Ok(entry.raw().0.parse::<Version>()?))
                    .collect::<Result<Vec<_>, tame_index::Error>>()
                    .map(Some)
            })
            .map_err(Into::into)
    }
}
