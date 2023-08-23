pub struct CratesIoIndex {
    index: tame_index::SparseIndex,
    client: reqwest::blocking::Client,
    etags: Vec<(String, String)>,
}

impl CratesIoIndex {
    #[inline]
    pub(crate) fn open() -> Result<Self, crate::error::CliError> {
        let index = tame_index::SparseIndex::new(tame_index::IndexLocation::new(
            tame_index::IndexUrl::CratesIoSparse,
        ))?;
        let client = reqwest::blocking::ClientBuilder::new()
            .http2_prior_knowledge()
            .build()?;

        Ok(Self {
            index,
            client,
            etags: Vec::new(),
        })
    }

    /// Determines if the specified crate exists in the crates.io index
    pub(crate) fn has_krate(&mut self, name: &str) -> Result<bool, crate::error::CliError> {
        Ok(self.has_krate_version(name, "")?.unwrap_or(false))
    }

    /// Determines if the specified crate version exists in the crates.io index
    pub(crate) fn has_krate_version(
        &mut self,
        name: &str,
        version: &str,
    ) -> Result<Option<bool>, crate::error::CliError> {
        let etag = self
            .etags
            .iter()
            .find_map(|(krate, etag)| (krate == name).then_some(etag.as_str()))
            .unwrap_or("");

        let krate_name = name.try_into()?;
        let req = self.index.make_remote_request(krate_name, Some(etag))?;
        let res = self.client.execute(req.try_into()?)?;

        // Grab the etag if it exists for future requests
        if let Some(etag) = res.headers().get(reqwest::header::ETAG) {
            if let Ok(etag) = etag.to_str() {
                if let Some(i) = self.etags.iter().position(|(krate, _)| krate == name) {
                    self.etags[i].1 = etag.to_owned();
                } else {
                    self.etags.push((name.to_owned(), etag.to_owned()));
                }
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

        let index_krate = self
            .index
            .parse_remote_response(krate_name, response, false)?;
        Ok(index_krate.map(|ik| ik.versions.iter().any(|iv| iv.version == version)))
    }
}
