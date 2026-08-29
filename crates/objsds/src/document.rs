use crate::{CompatibilityError, Error};

pub(crate) const FORMAT_VERSION: u32 = 1;

pub(crate) fn validate<E>(
    format_version: u32,
    observed_kind: &str,
    observed_schema: &str,
    expected_kind: &str,
    expected_schema: &str,
) -> Result<(), Error<E>> {
    if format_version != FORMAT_VERSION {
        return Err(Error::Incompatible(CompatibilityError::FormatVersion {
            expected: FORMAT_VERSION,
            observed: format_version,
        }));
    }
    if observed_kind != expected_kind {
        return Err(Error::Incompatible(CompatibilityError::Kind {
            expected: expected_kind.to_owned(),
            observed: observed_kind.to_owned(),
        }));
    }
    if observed_schema != expected_schema {
        return Err(Error::Incompatible(CompatibilityError::Schema {
            expected: expected_schema.to_owned(),
            observed: observed_schema.to_owned(),
        }));
    }
    Ok(())
}
