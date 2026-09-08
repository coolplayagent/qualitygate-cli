use super::Source;
use anyhow::{Result, bail};
use std::path::Path;

pub(super) fn id(id: &str) -> Result<()> {
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

pub(super) fn source(source: &Source) -> Result<()> {
    crate::paths::relative(Path::new(&source.document))?;
    if source.section.trim().is_empty()
        || !source
            .content_hash
            .strip_prefix("sha256:")
            .is_some_and(|hash| {
                hash.len() == 64
                    && hash
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            })
    {
        bail!("Rule source needs a nonempty section and lowercase sha256 content_hash");
    }
    Ok(())
}
