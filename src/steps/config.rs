use std::io::Write;

use crate::config::Config;
use crate::config::ConfigArgs;
use crate::config::load_package_config;
use crate::config::load_workspace_config;
use crate::error::CliError;

/// Dump workspace configuration
#[derive(Debug, Clone, clap::Args)]
pub struct ConfigStep {
    /// Write the current configuration to file with `-` for stdout
    #[arg(short, long, default_value = "-")]
    output: std::path::PathBuf,

    #[command(flatten)]
    manifest: clap_cargo::Manifest,

    #[command(flatten)]
    config: ConfigArgs,
}

impl ConfigStep {
    pub fn run(&self) -> Result<(), CliError> {
        log::trace!("initializing");
        let ws_meta = self
            .manifest
            .metadata()
            // When evaluating dependency ordering, we need to consider optional dependencies
            .features(cargo_metadata::CargoOpt::AllFeatures)
            .exec()?;

        let release_config =
            if let Some(root_id) = ws_meta.resolve.as_ref().and_then(|r| r.root.as_ref()) {
                let pkg = ws_meta
                    .packages
                    .iter()
                    .find(|p| p.id == *root_id)
                    .expect("root should always be present");

                let mut release_config = Config::from_defaults();
                release_config.update(&load_package_config(&self.config, &ws_meta, pkg)?);
                release_config
            } else {
                let mut release_config = Config::from_defaults();
                release_config.update(&load_workspace_config(&self.config, &ws_meta)?);
                release_config
            };

        let output = toml::to_string_pretty(&release_config)?;

        if self.output == std::path::Path::new("-") {
            std::io::stdout().write_all(output.as_bytes())?;
        } else {
            std::fs::write(&self.output, &output)?;
        }

        Ok(())
    }
}
