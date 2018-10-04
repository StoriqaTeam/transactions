use std::{fmt, fmt::Display};

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Facebook,
    Google,
}

impl Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Provider::Facebook => f.write_str("facebook"),
            Provider::Google => f.write_str("google"),
        }
    }
}
