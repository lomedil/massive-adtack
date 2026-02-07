use crate::UserCommands;
use crate::config::Config;
use crate::dn::DistinguishedName;
use crate::naming::NamingFormatter;
use anyhow::{Context, Result};
use fake::Fake;
use fake::faker::name::raw::*;
use fake::locales::EN;
use indicatif::{ProgressBar, ProgressStyle};
use ldap3::{Ldap, LdapConnAsync, LdapConnSettings, SearchEntry};
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

pub async fn execute(command: UserCommands) -> Result<()> {
    match command {
        UserCommands::Add {
            count,
            format,
            container,
        } => add_users(count, format, container).await,
        UserCommands::List {
            filter,
            container,
            ldap_filter,
        } => list_users(filter, container, ldap_filter).await,
        UserCommands::Rm {
            filter,
            container,
            dry_run,
            no_confirm,
        } => rm_users(filter, container, dry_run, no_confirm).await,
    }
}

async fn rm_users(
    filter: String,
    container: Option<DistinguishedName>,
    dry_run: bool,
    no_confirm: bool,
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

    // Filter construction
    // If filter contains '*', use as is. Otherwise correct equality match.
    let ldap_filter = format!(
        "(&(objectClass=user)({}={}))",
        cfg.mappings.username, filter
    );

    println!("Searching for users to remove...");
    println!("Base: {}", target_base);
    println!("Filter: {}\n", ldap_filter);

    let (res, _) = ldap
        .search(
            target_base.as_str(),
            ldap3::Scope::Subtree,
            &ldap_filter,
            vec!["dn"],
        )
        .await?
        .success()?;

    let count = res.len();
    if count == 0 {
        println!("No users found matching the filter.");
        return Ok(());
    }

    println!("Found {} users matching the filter.", count);

    // List preview (first 20)
    for (i, entry) in res.iter().take(20).enumerate() {
        let search_entry = SearchEntry::construct(entry.clone());
        println!("  {}: {}", i + 1, search_entry.dn);
    }
    if count > 20 {
        println!("  ...and {} more.", count - 20);
    }

    if dry_run {
        println!("\nDry run enabled. No changes made.");
        return Ok(());
    }

    // Confirmation
    if !no_confirm {
        print!(
            "\nAre you sure you want to delete these {} users? [y/N] ",
            count
        );
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Operation cancelled.");
            return Ok(());
        }
    }

    println!("\nDeleting users...");
    let pb = ProgressBar::new(count as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.red/white}] {pos}/{len} ({eta}) {msg}")?
            .progress_chars("#>-"),
    );

    let mut success = 0;
    let mut failures = 0;

    for entry in res {
        let search_entry = SearchEntry::construct(entry);
        let dn = search_entry.dn;

        pb.set_message(format!("Deleting {}", dn));

        match ldap.delete(&dn).await {
            Ok(res) => {
                if res.clone().success().is_ok() {
                    success += 1;
                } else {
                    failures += 1;
                    pb.println(format!("Failed to delete {}: {:?}", dn, res));
                }
            }
            Err(e) => {
                failures += 1;
                pb.println(format!("Error deleting {}: {}", dn, e));
            }
        }
        pb.inc(1);
    }

    pb.finish_with_message("Done");

    println!("\nDeleted {}/{} users.", success, count);
    if failures > 0 {
        println!("Failed to delete {} users.", failures);
    }

    Ok(())
}

async fn add_users(
    count: u32,
    template_override: Option<String>,
    container: Option<DistinguishedName>,
) -> Result<()> {
    let cfg = Config::load()?;
    let domain = cfg
        .base_dn
        .dns_name()
        .context("Could not find a DC= part in base DN")?;
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

    println!("Generating {} users for domain: {}\n", count, domain);

    let pb = ProgressBar::new(count as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")?
            .progress_chars("#>-"),
    );

    let mut success = 0;
    let mut failure = 0;
    let start_time = Instant::now();

    for i in 1..=count {
        let (dn, attrs) = prepare_user_entry(&cfg, &formatter, &domain, &target_base, i);
        pb.set_message(format!("Adding {}", dn));

        let res = ldap.add(dn.as_str(), attrs).await?;
        match res.success() {
            Ok(_) => success += 1,
            Err(e) => {
                failure += 1;
                pb.suspend(|| {
                    eprintln!("  Warning: Failed to add user {}: {}", dn, e);
                });
            }
        }
        pb.inc(1);
    }

    pb.finish_with_message("Done!");

    let total_duration = start_time.elapsed();
    let rate = success as f64 / total_duration.as_secs_f64();

    println!("\n--- Execution Summary ---");
    println!("Total Time:       {:?}", total_duration);
    println!("Successful:       {}", success);
    println!("Failed:           {}", failure);
    println!("Creation Rate:    {:.2} users/sec", rate);

    ldap.unbind().await.context("Failed to unbind")?;
    Ok(())
}

async fn list_users(
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
        // Smart filter: search in common attributes
        format!(
            "(&(objectClass=user)(|(cn={0})(sAMAccountName={0})(mail={0})))",
            pattern
        )
    } else {
        "(objectClass=user)".to_string()
    };

    println!("Searching in base: {}", target_base);
    println!("Filter: {}\n", final_filter);

    let (res, _) = ldap
        .search(
            target_base.as_str(),
            ldap3::Scope::Subtree,
            &final_filter,
            vec![
                &cfg.mappings.username,
                &cfg.mappings.first_name,
                &cfg.mappings.last_name,
                &cfg.mappings.email,
            ],
        )
        .await?
        .success()?;

    if res.is_empty() {
        println!("No users found.");
        return Ok(());
    }

    // Header
    println!(
        "{:<20} {:<30} {:<30} {:<30}",
        "Username", "First Name", "Last Name", "Email"
    );
    println!("{}", "-".repeat(110));

    for entry in res {
        let search_entry = SearchEntry::construct(entry);
        let username = get_attr(&search_entry, &cfg.mappings.username);
        let first_name = get_attr(&search_entry, &cfg.mappings.first_name);
        let last_name = get_attr(&search_entry, &cfg.mappings.last_name);
        let email = get_attr(&search_entry, &cfg.mappings.email);

        println!(
            "{:<20} {:<30} {:<30} {:<30}",
            username, first_name, last_name, email
        );
    }

    Ok(())
}

fn get_attr(entry: &ldap3::SearchEntry, attr: &str) -> String {
    entry
        .attrs
        .get(attr)
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default()
}

async fn connect_ldap(cfg: &Config) -> Result<Ldap> {
    println!("Connecting to {}...", cfg.url);
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
        HashSet::from_iter([username.clone()]),
    );
    attrs.insert(
        cfg.mappings.first_name.clone(),
        HashSet::from_iter([first_name.clone()]),
    );
    attrs.insert(
        cfg.mappings.last_name.clone(),
        HashSet::from_iter([last_name.clone()]),
    );
    attrs.insert(
        cfg.mappings.email.clone(),
        HashSet::from_iter([email.clone()]),
    );
    attrs.insert(cfg.mappings.phone.clone(), HashSet::from_iter([phone]));

    // AD specific fields often required
    attrs.insert("userPrincipalName".to_string(), HashSet::from_iter([email]));
    attrs.insert(
        "displayName".to_string(),
        HashSet::from_iter([format!("{} {}", first_name, last_name)]),
    );
    attrs.insert("cn".to_string(), HashSet::from_iter([username]));

    (dn, attrs.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_name_extraction() {
        assert_eq!(
            DistinguishedName::try_from("DC=lab,DC=internal")
                .unwrap()
                .dns_name()
                .unwrap(),
            "lab.internal"
        );
        assert_eq!(
            DistinguishedName::try_from("OU=Users,DC=example,DC=com")
                .unwrap()
                .dns_name()
                .unwrap(),
            "example.com"
        );
        assert_eq!(
            DistinguishedName::try_from("CN=Users,DC=corp")
                .unwrap()
                .dns_name()
                .unwrap(),
            "corp"
        );
    }
}
