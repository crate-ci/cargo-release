use std::collections::BTreeMap;
use std::path::Path;

use fancy_regex::RegexBuilder;

use crate::config::Replace;
use crate::error::FatalError;

#[derive(Clone, Default, Debug)]
pub struct Template<'a> {
    pub prev_version: Option<&'a str>,
    pub prev_metadata: Option<&'a str>,
    pub version: Option<&'a str>,
    pub metadata: Option<&'a str>,
    pub crate_name: Option<&'a str>,
    pub date: Option<&'a str>,

    pub prefix: Option<&'a str>,
    pub tag_name: Option<&'a str>,
    pub next_version: Option<&'a str>,
    pub next_metadata: Option<&'a str>,
}

impl<'a> Template<'a> {
    pub fn render(&self, input: &str) -> String {
        let mut s = input.to_string();
        if let Some(prev_version) = self.prev_version {
            s = s.replace("{{prev_version}}", prev_version);
        }
        if let Some(prev_metadata) = self.prev_metadata {
            s = s.replace("{{prev_metadata}}", prev_metadata);
        }
        if let Some(version) = self.version {
            s = s.replace("{{version}}", version);
        }
        if let Some(metadata) = self.metadata {
            s = s.replace("{{metadata}}", metadata);
        }
        if let Some(crate_name) = self.crate_name {
            s = s.replace("{{crate_name}}", crate_name);
        }
        if let Some(date) = self.date {
            s = s.replace("{{date}}", date);
        }

        if let Some(prefix) = self.prefix {
            s = s.replace("{{prefix}}", prefix);
        }
        if let Some(tag_name) = self.tag_name {
            s = s.replace("{{tag_name}}", tag_name);
        }
        if let Some(next_version) = self.next_version {
            s = s.replace("{{next_version}}", next_version);
        }
        if let Some(next_metadata) = self.next_metadata {
            s = s.replace("{{next_metadata}}", next_metadata);
        }
        s
    }
}

pub fn do_file_replacements(
    replace_config: &[Replace],
    template: &Template<'_>,
    cwd: &Path,
    prerelease: bool,
    dry_run: bool,
) -> Result<bool, FatalError> {
    // Since we don't have a convenient insert-order map, let's do sorted, rather than random.
    let mut by_file = BTreeMap::new();
    for replace in replace_config {
        let file = replace.file.clone();
        by_file.entry(file).or_insert_with(Vec::new).push(replace);
    }

    for (path, replaces) in by_file.into_iter() {
        let file = cwd.join(&path);
        log::debug!("Substituting values for {}", file.display());
        if !file.exists() {
            return Err(FatalError::FileNotFound(file));
        }
        let data = std::fs::read_to_string(&file)?;
        let mut replaced = data.clone();

        for replace in replaces {
            if prerelease && !replace.prerelease {
                log::debug!("Pre-release, not replacing {}", replace.search);
                continue;
            }

            let pattern = replace.search.as_str();
            let r = RegexBuilder::new(pattern)
                // .multi_line(true) Not available with fancy_regex
                .build()
                .map_err(FatalError::from)?;

            let min = replace.min.or(replace.exactly).unwrap_or(1);
            let max = replace.max.or(replace.exactly).unwrap_or(std::usize::MAX);
            let actual = r.find_iter(&replaced).count();
            if actual < min {
                return Err(FatalError::ReplacerMinError(
                    pattern.to_owned(),
                    min,
                    actual,
                ));
            } else if max < actual {
                return Err(FatalError::ReplacerMaxError(
                    pattern.to_owned(),
                    min,
                    actual,
                ));
            }

            let to_replace = replace.replace.as_str();
            let replacer = template.render(to_replace);

            replaced = r.replace_all(&replaced, replacer.as_str()).into_owned();
        }

        if data != replaced {
            if dry_run {
                let display_path = path.display().to_string();
                let data_lines: Vec<_> = data.lines().map(|s| format!("{}\n", s)).collect();
                let replaced_lines: Vec<_> = replaced.lines().map(|s| format!("{}\n", s)).collect();
                let diff = difflib::unified_diff(
                    &data_lines,
                    &replaced_lines,
                    display_path.as_str(),
                    display_path.as_str(),
                    "original",
                    "replaced",
                    0,
                );
                log::debug!("Change:\n{}", itertools::join(diff.into_iter(), ""));
            } else {
                std::fs::write(&file, replaced)?;
            }
        }
    }
    Ok(true)
}
