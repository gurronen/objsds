use std::fmt;

/// Credentials used to sign S3 requests.
#[derive(Clone, Default, Eq, PartialEq)]
pub enum Credentials {
    /// Resolve credentials using the adapter's standard provider chain.
    #[default]
    Default,
    /// Use explicitly supplied credentials.
    Static {
        /// Access-key identifier included in signed requests.
        access_key_id: String,
        /// Secret signing key. Debug output always redacts this value.
        secret_access_key: String,
        /// Optional temporary-credential token, also redacted from debug output.
        session_token: Option<String>,
    },
}

impl Credentials {
    pub(crate) fn to_auth(&self) -> Result<s3_client::Auth, crate::BuildError> {
        match self {
            Self::Default => Ok(s3_client::Auth::from_env()?),
            Self::Static {
                access_key_id,
                secret_access_key,
                session_token,
            } => {
                let credentials = s3_client::Credentials::new(access_key_id, secret_access_key)?;
                let credentials = match session_token {
                    Some(token) => credentials.with_session_token(token)?,
                    None => credentials,
                };
                Ok(s3_client::Auth::Static(credentials))
            }
        }
    }

    /// Creates static credentials without a session token.
    #[must_use]
    pub fn new(access_key_id: impl Into<String>, secret_access_key: impl Into<String>) -> Self {
        Self::Static {
            access_key_id: access_key_id.into(),
            secret_access_key: secret_access_key.into(),
            session_token: None,
        }
    }

    /// Adds a session token to static credentials.
    ///
    /// Calling this on [`Credentials::Default`] has no effect.
    #[must_use]
    pub fn with_session_token(mut self, token: impl Into<String>) -> Self {
        if let Self::Static { session_token, .. } = &mut self {
            *session_token = Some(token.into());
        }
        self
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => formatter.write_str("Default"),
            Self::Static {
                access_key_id,
                session_token,
                ..
            } => formatter
                .debug_struct("Static")
                .field("access_key_id", access_key_id)
                .field("secret_access_key", &"[REDACTED]")
                .field(
                    "session_token",
                    &session_token.as_ref().map(|_| "[REDACTED]"),
                )
                .finish(),
        }
    }
}
