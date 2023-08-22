pub(crate) fn open_crates_io_index() -> Result<tame_index::index::ComboIndex, crate::error::CliError>
{
    let index = tame_index::index::ComboIndexCache::new(tame_index::IndexLocation::new(
        tame_index::IndexUrl::crates_io(None, None, None)?,
    ))?;

    let index = match index {
        tame_index::index::ComboIndexCache::Git(gi) => {
            tame_index::index::RemoteGitIndex::new(gi)?.into()
        }
        tame_index::index::ComboIndexCache::Sparse(si) => {
            tame_index::index::RemoteSparseIndex::new(
                si,
                tame_index::external::reqwest::blocking::Client::builder()
                    .http2_prior_knowledge()
                    .build()
                    .map_err(tame_index::Error::from)?,
            )
            .into()
        }
    };

    Ok(index)
}
