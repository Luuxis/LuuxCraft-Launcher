//! One error type for every command.
//!
//! The frontend only sees a stable `code` (translated into a readable message)
//! plus a `message`/`details` pair meant for the logs and the "technical
//! details" disclosure. Secrets never go through here.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    pub fn unknown(message: impl Into<String>) -> Self {
        Self::new("unknown", message)
    }

    pub fn cancelled() -> Self {
        Self::new("cancelled", "operation cancelled")
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.details {
            Some(details) => write!(f, "[{}] {} ({details})", self.code, self.message),
            None => write!(f, "[{}] {}", self.code, self.message),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::new("io", error.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::new("api_invalid", format!("invalid json: {error}"))
    }
}

impl From<tauri::Error> for AppError {
    fn from(error: tauri::Error) -> Self {
        Self::new("internal", error.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::new("timeout", error.to_string())
        } else if error.is_connect() || error.is_request() {
            Self::new("network", error.to_string())
        } else if error.is_decode() {
            Self::new("api_invalid", error.to_string())
        } else {
            Self::new("network", error.to_string())
        }
    }
}

impl From<crust_core::network::Error> for AppError {
    fn from(error: crust_core::network::Error) -> Self {
        use crust_core::network::Error;
        match error {
            Error::Transport(inner) => Self::from(inner),
            Error::Build(inner) => Self::new("internal", inner.to_string()),
            Error::Decode(inner) => Self::new("api_invalid", inner.to_string()),
            Error::Status { status, url } => {
                let code = if status >= 500 {
                    "api_unavailable"
                } else if status == 404 {
                    "api_not_found"
                } else {
                    "api_invalid"
                };
                Self::new(code, format!("unexpected status {status} for {url}"))
            }
            Error::Io { path, source } => Self::new("io", format!("{source}")).with_details(path),
            Error::Download {
                url,
                attempts,
                source,
            } => Self::new(
                "download_failed",
                format!("download failed after {attempts} attempts"),
            )
            .with_details(format!("{url}: {source}")),
            Error::Join(inner) => Self::new("internal", inner.to_string()),
        }
    }
}

impl From<crust_core::network::status::Error> for AppError {
    fn from(error: crust_core::network::status::Error) -> Self {
        use crust_core::network::status::Error;
        match error {
            Error::Timeout(address) => {
                Self::new("server_unreachable", format!("timeout reaching {address}"))
            }
            other => Self::new("server_unreachable", other.to_string()),
        }
    }
}

impl From<crust_core::authenticator::Error> for AppError {
    fn from(error: crust_core::authenticator::Error) -> Self {
        use crust_core::authenticator::Error;
        match error {
            Error::Network(inner) => Self::from(inner),
            Error::DeviceCodeDeclined => Self::new("auth_denied", error.to_string()),
            // The device code flow is no longer offered; the variant stays for
            // exhaustiveness (and for the diagnostic test).
            Error::DeviceCodeExpired => Self::new("auth_code_expired", error.to_string()),
            Error::MissingRefreshToken => Self::new("auth_expired", error.to_string()),
            Error::NoProfile { .. } => Self::new("auth_no_profile", error.to_string()),
            Error::AuthorizationDenied { .. }
            | Error::StateMismatch
            | Error::InvalidRedirectUrl => Self::new("auth_denied", error.to_string()),
            Error::Microsoft(inner) => {
                let text = inner.to_string();
                let code = if text.contains("invalid_grant") || text.contains("expired") {
                    "auth_expired"
                } else {
                    "auth_denied"
                };
                Self::new(code, text)
            }
            Error::AzAuth(inner) => match inner {
                crust_core::providers::azauth::Error::Rejected { reason, message } => {
                    let lowered = reason.to_ascii_lowercase();
                    let code = if lowered.contains("2fa") || lowered.contains("code") {
                        "auth_otp_invalid"
                    } else if lowered.contains("ban") {
                        "auth_banned"
                    } else if lowered.contains("credential") || lowered.contains("password") {
                        "auth_invalid_credentials"
                    } else if lowered.contains("token") {
                        "auth_expired"
                    } else {
                        "auth_denied"
                    };
                    Self::new(code, message.unwrap_or(reason.clone())).with_details(reason)
                }
                crust_core::providers::azauth::Error::Network(inner) => Self::from(inner),
                crust_core::providers::azauth::Error::Unexpected { status, body } => Self::new(
                    "api_invalid",
                    format!("unexpected AZauth response (status {status})"),
                )
                .with_details(truncate(&body, 300)),
            },
            Error::Yggdrasil(inner) => {
                let text = inner.to_string();
                let code = if text.to_ascii_lowercase().contains("invalid") {
                    "auth_invalid_credentials"
                } else {
                    "auth_denied"
                };
                Self::new(code, text)
            }
            Error::Xbox(inner) => Self::new("auth_denied", inner.to_string()),
            Error::Minecraft(inner) => Self::new("auth_denied", inner.to_string()),
            Error::InvalidUrl(inner) => Self::new("config_invalid", inner.to_string()),
            Error::MissingUserHash => Self::new("auth_denied", error.to_string()),
        }
    }
}

impl From<crust_core::launcher::Error> for AppError {
    fn from(error: crust_core::launcher::Error) -> Self {
        use crust_core::launcher::Error;
        match error {
            Error::Network(inner) => Self::from(inner),
            Error::Resolver(inner) => match inner {
                crust_core::resolver::Error::VersionNotFound(version) => {
                    Self::new("instance_invalid", format!("Minecraft {version} not found"))
                }
                crust_core::resolver::Error::CustomFiles { url, status } => Self::new(
                    "api_unavailable",
                    format!("instance files unavailable (status {status})"),
                )
                .with_details(url),
                crust_core::resolver::Error::Network(inner) => Self::from(inner),
                crust_core::resolver::Error::Meta(inner) => Self::new("network", inner.to_string()),
                crust_core::resolver::Error::MissingClient => {
                    Self::new("instance_invalid", inner.to_string())
                }
                other => Self::new("java_missing", other.to_string()),
            },
            Error::Loader(inner) => Self::new("install_failed", inner.to_string()),
            Error::LoaderNotFound(name) => {
                Self::new("instance_invalid", format!("unknown loader {name}"))
            }
            Error::NoJava | Error::JavaUnavailable(_) | Error::Azul(_) => {
                Self::new("java_missing", error.to_string())
            }
            Error::NoMainClass => Self::new("launch_failed", error.to_string()),
            Error::Io { path, source } => Self::new("io", source.to_string()).with_details(path),
            Error::Zip { path, source } => {
                Self::new("file_corrupted", source.to_string()).with_details(path)
            }
            Error::Join(inner) => Self::new("internal", inner.to_string()),
        }
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let cut: String = text.chars().take(max).collect();
        format!("{cut}…")
    }
}
