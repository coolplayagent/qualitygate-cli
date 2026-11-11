use crate::config::Source;
use anyhow::{Result, bail};
use std::path::Path;

pub(crate) fn id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        bail!("Invalid check id: {id}");
    }
    Ok(())
}

pub(crate) fn source(source: &Source) -> Result<()> {
    crate::paths::relative(Path::new(&source.document))?;
    if source.section.trim().is_empty() || !crate::domain::digest::is_sha256(&source.content_hash) {
        bail!("Rule source needs a nonempty section and lowercase sha256 content_hash");
    }
    Ok(())
}
