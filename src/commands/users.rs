use crate::UserCommands;
use crate::config::Config;
use crate::naming::NamingFormatter;
use anyhow::{Context, Result};
use fake::Fake;
use fake::faker::name::raw::*;
use fake::locales::EN;
use ldap3::{Ldap, LdapConnAsync, LdapConnSettings};
use std::collections::{BTreeMap, HashSet};

pub async fn execute(command: UserCommands) -> Result<()> {
    match command {
        UserCommands::Add { count, format } => add_users(count, format).await,
    }
}

async fn add_users(count: u32, template_override: Option<String>) -> Result<()> {
    let cfg = Config::load()?;
    let domain = derive_domain(&cfg.base_dn);
    let formatter = NamingFormatter::new(template_override.or(cfg.username_format.clone()));

    let mut ldap = connect_ldap(&cfg).await?;

    println!("Generating {} users for domain: {}", count, domain);

    for i in 1..=count {
        let (dn, attrs) = prepare_user_entry(&cfg, &formatter, &domain, i);

        println!("[{}/{}] Adding user: {} ", i, count, dn);

        let res = ldap.add(&dn, attrs).await?;
        if let Err(e) = res.success() {
            eprintln!("  Warning: Failed to add user at index {}: {}", i, e);
        }
    }

    ldap.unbind().await.context("Failed to unbind")?;
    println!("\nUser creation completed.");
    Ok(())
}

async fn connect_ldap(cfg: &Config) -> Result<Ldap> {
    println!("Connecting to {}...", cfg.url);
    let mut settings = LdapConnSettings::new();
    if let Some(ca_cert) = &cfg.tls_ca_cert {
        if ca_cert == "never" {
            settings = settings.set_no_tls_verify(true);
        }
    }

    let (conn, mut ldap) = LdapConnAsync::with_settings(settings, &cfg.url)
        .await
        .context("Failed to connect to LDAP server")?;
    ldap3::drive!(conn);

    ldap.simple_bind(&cfg.user, &cfg.password)
        .await
        .context("Failed to bind to LDAP server")?
        .success()
        .context("LDAP Bind operation failed")?;

    Ok(ldap)
}

fn prepare_user_entry(
    cfg: &Config,
    formatter: &NamingFormatter,
    domain: &str,
    index: u32,
) -> (String, Vec<(String, HashSet<String>)>) {
    let first_name: String = FirstName(EN).fake();
    let last_name: String = LastName(EN).fake();

    let username = formatter.generate(&first_name, &last_name, index);
    let email = format!("{}@{}", username, domain);
    let phone: String = format!("+34 6{:08}", (index % 100000000));

    // Use CN as RDN for AD compatibility
    let dn = format!("cn={},{}", username, cfg.base_dn);

    let mut attrs = BTreeMap::new();
    attrs.insert(
        "objectClass".to_string(),
        HashSet::from_iter(vec![
            "top".to_string(),
            "person".to_string(),
            "organizationalPerson".to_string(),
            "user".to_string(),
        ]),
    );
    attrs.insert(
        cfg.mappings.username.clone(),
        HashSet::from_iter(vec![username.clone()]),
    );
    attrs.insert(
        cfg.mappings.first_name.clone(),
        HashSet::from_iter(vec![first_name.clone()]),
    );
    attrs.insert(
        cfg.mappings.last_name.clone(),
        HashSet::from_iter(vec![last_name.clone()]),
    );
    attrs.insert(
        cfg.mappings.email.clone(),
        HashSet::from_iter(vec![email.clone()]),
    );
    attrs.insert(
        cfg.mappings.phone.clone(),
        HashSet::from_iter(vec![phone.clone()]),
    );

    // AD specific fields often required
    attrs.insert(
        "userPrincipalName".to_string(),
        HashSet::from_iter(vec![email.clone()]),
    );
    attrs.insert(
        "displayName".to_string(),
        HashSet::from_iter(vec![format!("{} {}", first_name, last_name)]),
    );
    attrs.insert("cn".to_string(), HashSet::from_iter(vec![username.clone()]));

    (dn, attrs.into_iter().collect())
}

fn derive_domain(base_dn: &str) -> String {
    base_dn
        .split(',')
        .filter(|part| part.to_uppercase().starts_with("DC="))
        .map(|part| &part[3..])
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_domain() {
        assert_eq!(derive_domain("DC=lab,DC=internal"), "lab.internal");
        assert_eq!(derive_domain("OU=Users,DC=example,DC=com"), "example.com");
        assert_eq!(derive_domain("CN=Users,DC=corp"), "corp");
    }
}
