use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DistinguishedName(String);

impl DistinguishedName {
    pub fn builder() -> DistinguishedNameBuilder {
        DistinguishedNameBuilder::new()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for DistinguishedName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for DistinguishedName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DistinguishedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for DistinguishedName {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

impl TryFrom<&str> for DistinguishedName {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self> {
        if s.is_empty() {
            return Err(anyhow!("Distinguished Name cannot be empty"));
        }
        // Basic validation: must contain at least one '='
        if !s.contains('=') {
            return Err(anyhow!("Invalid DN format: '{}' (must contain '=')", s));
        }
        Ok(DistinguishedName(s.to_string()))
    }
}

impl TryFrom<String> for DistinguishedName {
    type Error = anyhow::Error;

    fn try_from(s: String) -> Result<Self> {
        Self::try_from(s.as_str())
    }
}

#[derive(Default)]
pub struct DistinguishedNameBuilder {
    parts: Vec<String>,
    base: Option<String>,
}

impl DistinguishedNameBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, attr: &str, value: &str) -> &mut Self {
        self.parts.push(format!("{}={}", attr, value));
        self
    }

    pub fn add_raw(&mut self, raw: &str) -> &mut Self {
        self.parts.push(raw.to_string());
        self
    }

    pub fn append_base(&mut self, base: &DistinguishedName) -> &mut Self {
        self.base = Some(base.to_string());
        self
    }

    pub fn build(&self) -> Result<DistinguishedName> {
        let mut dn_str = self.parts.join(",");

        if let Some(base) = &self.base {
            if !dn_str.is_empty() {
                dn_str.push(',');
            }
            dn_str.push_str(base);
        }

        DistinguishedName::try_from(dn_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_dn() {
        let dn = DistinguishedName::try_from("cn=user,dc=example,dc=com").unwrap();
        assert_eq!(dn.as_str(), "cn=user,dc=example,dc=com");
    }

    #[test]
    fn test_invalid_dn() {
        assert!(DistinguishedName::try_from("").is_err());
        assert!(DistinguishedName::try_from("invalid").is_err());
    }

    #[test]
    fn test_builder() {
        let base = DistinguishedName::try_from("dc=example,dc=com").unwrap();
        let dn = DistinguishedName::builder()
            .add("cn", "john")
            .append_base(&base)
            .build()
            .unwrap();
        assert_eq!(dn.as_str(), "cn=john,dc=example,dc=com");
    }

    #[test]
    fn test_builder_no_base() {
        let dn = DistinguishedName::builder()
            .add("dc", "lab")
            .add("dc", "internal")
            .build()
            .unwrap();
        assert_eq!(dn.as_str(), "dc=lab,dc=internal");
    }
}
