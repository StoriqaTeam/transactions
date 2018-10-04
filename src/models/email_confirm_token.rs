use std::fmt;
use std::fmt::Display;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmailConfirmToken(String);

impl Display for EmailConfirmToken {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}
