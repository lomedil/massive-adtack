use crate::UserCommands;
use crate::config::Config;
use crate::dn::DistinguishedName;
use crate::naming::NamingFormatter;
use anyhow::{Context, Result};
use fake::Fake;
use fake::faker::name::raw::*;
use fake::locales::EN;
use ldap3::{Ldap, LdapConnAsync, LdapConnSettings};
use std::collections::{BTreeMap, HashSet};

pub async fn execute(command: UserCommands) -> Result<()> {
    match command {
        UserCommands::Add {
            count,
            format,
            container,
        } => add_users(count, format, container).await,
    }
}

async fn add_users(
    count: u32,
    template_override: Option<String>,
    container: Option<DistinguishedName>,
) -> Result<()> {
    let cfg = Config::load()?;
    let domain = derive_domain(&cfg.base_dn);
    let formatter = NamingFormatter::new(template_override.or(cfg.username_format.clone()));

    let mut ldap = connect_ldap(&cfg).await?;

    // Determine the full base DN for users
    let target_base = if let Some(c) = &container {
        DistinguishedName::builder()
            .add_raw(c.as_str())
            .append_base(&cfg.base_dn)
            .build()?
    } else {
        cfg.base_dn.clone()
    };

    println!("Validating target base: {}", target_base);
    validate_base_exists(&mut ldap, &target_base).await?;

    println!("Generating {} users for domain: {}", count, domain);

    for i in 1..=count {
        let (dn, attrs) = prepare_user_entry(&cfg, &formatter, &domain, &target_base, i);

        println!("[{}/{}] Adding user: {} ", i, count, dn);

        let res = ldap.add(&dn.to_string(), attrs).await?;
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

    ldap.simple_bind(cfg.user.as_str(), &cfg.password)
        .await
        .context("Failed to bind to LDAP server")?
        .success()
        .context("LDAP Bind operation failed")?;

    Ok(ldap)
}

async fn validate_base_exists(ldap: &mut Ldap, base_dn: &DistinguishedName) -> Result<()> {
    use ldap3::Scope;

    let res = ldap
        .search(
            base_dn.as_str(),
            Scope::Base,
            "(objectClass=*)",
            vec!["1.1"],
        )
        .await
        .context(format!("Failed to search for base DN: {}", base_dn))?;

    let (_entries, result) = res.success().context(format!(
        "LDAP error while validating container '{}'",
        base_dn
    ))?;

    if result.rc != 0 {
        return Err(anyhow::anyhow!(
            "Target container '{}' does not exist or is not accessible (RC={})",
            base_dn,
            result.rc
        ));
    }

    Ok(())
}

fn prepare_user_entry(
    cfg: &Config,
    formatter: &NamingFormatter,
    domain: &str,
    target_base: &DistinguishedName,
    index: u32,
) -> (DistinguishedName, Vec<(String, HashSet<String>)>) {
    let first_name: String = FirstName(EN).fake();
    let last_name: String = LastName(EN).fake();

    let username = formatter.generate(&first_name, &last_name, index);
    let email = format!("{}@{}", username, domain);
    let phone: String = format!("+34 6{:08}", (index % 100000000));

    // Use CN as RDN for AD compatibility, within the target base
    let dn = DistinguishedName::builder()
        .add("cn", &username)
        .append_base(target_base)
        .build()
        .expect("Failed to build user DN");

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

fn derive_domain(base_dn: &DistinguishedName) -> String {
    base_dn
        .as_str()
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
        assert_eq!(
            derive_domain(&DistinguishedName::try_from("DC=lab,DC=internal").unwrap()),
            "lab.internal"
        );
        assert_eq!(
            derive_domain(&DistinguishedName::try_from("OU=Users,DC=example,DC=com").unwrap()),
            "example.com"
        );
        assert_eq!(
            derive_domain(&DistinguishedName::try_from("CN=Users,DC=corp").unwrap()),
            "corp"
        );
    }
}
