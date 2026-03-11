use crate::GroupCommands;
use crate::config::Config;
use crate::dn::DistinguishedName;
use anyhow::{Context, Result};
use ldap3::{Ldap, LdapConnAsync, LdapConnSettings, SearchEntry};

use std::collections::{BTreeMap, HashSet};

pub async fn execute(command: GroupCommands) -> Result<()> {
    match command {
        GroupCommands::Add {
            groupname,
            container,
        } => add_group(groupname, container).await,
        GroupCommands::List {
            filter,
            container,
            ldap_filter,
        } => list_groups(filter, container, ldap_filter).await,
    }
}

async fn list_groups(
    filter: Option<String>,
    container: Option<DistinguishedName>,
    ldap_filter: Option<String>,
) -> Result<()> {
    let cfg = Config::load()?;
    let mut ldap = connect_ldap(&cfg).await?;

    let target_base = if let Some(c) = &container {
        DistinguishedName::builder()
            .add_raw(c.as_str())
            .append_base(&cfg.base_dn)
            .build()?
    } else {
        cfg.base_dn.clone()
    };

    let final_filter = if let Some(raw) = ldap_filter {
        raw
    } else if let Some(f) = filter {
        let pattern = if f.contains('*') {
            f
        } else {
            format!("*{}*", f)
        };
        format!(
            "(&(objectClass=group)(|(cn={0})(sAMAccountName={0})))",
            pattern
        )
    } else {
        "(objectClass=group)".to_string()
    };

    println!("Searching groups in base: {}", target_base);
    println!("Filter: {}\n", final_filter);

    let (res, _) = ldap
        .search(
            target_base.as_str(),
            ldap3::Scope::Subtree,
            &final_filter,
            vec!["sAMAccountName", "cn", "member"],
        )
        .await?
        .success()?;

    if res.is_empty() {
        println!("No groups found.");
        return Ok(());
    }

    // Header
    println!("{:<15} {:<25} {:<10}", "Name", "CN", "Members");
    println!("{}", "-".repeat(55));

    for entry in res {
        let search_entry = SearchEntry::construct(entry);
        let name = get_attr(&search_entry, "sAMAccountName");
        let cn = get_attr(&search_entry, "cn");

        // Count members
        let member_count = search_entry
            .attrs
            .get("member")
            .map(|v| v.len())
            .unwrap_or(0);

        println!("{:<15} {:<25} {:<10}", name, cn, member_count);
    }

    Ok(())
}

async fn add_group(groupname: String, container: Option<DistinguishedName>) -> Result<()> {
    let cfg = Config::load()?;
    let mut ldap = connect_ldap(&cfg).await?;

    let target_base = if let Some(c) = &container {
        DistinguishedName::builder()
            .add_raw(c.as_str())
            .append_base(&cfg.base_dn)
            .build()?
    } else {
        cfg.base_dn.clone()
    };

    println!("Validating target base: {}", target_base);
    if let Err(e) = crate::commands::users::validate_base_exists(&mut ldap, &target_base).await {
        anyhow::bail!("Error validating target container '{}': {}", target_base, e);
    }

    println!("Creating group '{}' in: {}", groupname, target_base);

    let dn = DistinguishedName::builder()
        .add("cn", &groupname)
        .append_base(&target_base)
        .build()
        .context("Failed to build group DN")?;

    let mut attrs = BTreeMap::new();
    attrs.insert(
        "objectClass".to_string(),
        HashSet::from_iter(vec!["top".to_string(), "group".to_string()]),
    );
    attrs.insert(
        "sAMAccountName".to_string(),
        HashSet::from_iter(vec![groupname.clone()]),
    );
    attrs.insert(
        "cn".to_string(),
        HashSet::from_iter(vec![groupname.clone()]),
    );

    let res = ldap.add(dn.as_str(), attrs.into_iter().collect()).await?;
    match res.success() {
        Ok(_) => {
            println!("Successfully created group '{}'", groupname);
            Ok(())
        }
        Err(e) => {
            anyhow::bail!(
                "Failed to create group '{}'. AD Server returned: {}",
                groupname,
                e
            );
        }
    }
}

fn get_attr(entry: &SearchEntry, attr: &str) -> String {
    entry
        .attrs
        .get(attr)
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default()
}

async fn connect_ldap(cfg: &Config) -> Result<Ldap> {
    let mut settings = LdapConnSettings::new();
    if cfg.tls_ca_cert.as_deref() == Some("never") {
        settings = settings.set_no_tls_verify(true);
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
