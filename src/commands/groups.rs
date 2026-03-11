use crate::GroupCommands;
use crate::config::Config;
use crate::dn::DistinguishedName;
use anyhow::{Context, Result, bail};
use ldap3::{Ldap, LdapConnAsync, LdapConnSettings, Mod, Scope, SearchEntry};

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
        GroupCommands::Rm {
            name,
            container,
            dry_run,
            no_confirm,
        } => rm_group(name, container, dry_run, no_confirm).await,
        GroupCommands::Join {
            name,
            filter,
            ldap_filter,
            container,
            dry_run,
        } => join_group(name, filter, ldap_filter, container, dry_run).await,
        GroupCommands::Leave {
            name,
            filter,
            ldap_filter,
            container,
            dry_run,
        } => leave_group(name, filter, ldap_filter, container, dry_run).await,
    }
}

enum MembershipOperation {
    Join,
    Leave,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GroupLookup {
    FullDn(DistinguishedName),
    RdnCn(String),
    SamAccountName(String),
}

impl GroupLookup {
    fn input_type(&self) -> &'static str {
        match self {
            Self::FullDn(_) => "full DN",
            Self::RdnCn(_) => "RDN (CN=...)",
            Self::SamAccountName(_) => "sAMAccountName",
        }
    }

    fn lookup_attribute(&self) -> &'static str {
        match self {
            Self::FullDn(_) => "distinguishedName",
            Self::RdnCn(_) => "cn",
            Self::SamAccountName(_) => "sAMAccountName",
        }
    }

    fn original_value(&self) -> &str {
        match self {
            Self::FullDn(dn) => dn.as_str(),
            Self::RdnCn(value) | Self::SamAccountName(value) => value,
        }
    }

    fn ldap_filter(&self) -> String {
        match self {
            Self::FullDn(dn) => format!(
                "(&(objectClass=group)(distinguishedName={}))",
                escape_ldap_filter_value(dn.as_str())
            ),
            Self::RdnCn(value) => format!(
                "(&(objectClass=group)(cn={}))",
                escape_ldap_filter_value(value)
            ),
            Self::SamAccountName(value) => format!(
                "(&(objectClass=group)(sAMAccountName={}))",
                escape_ldap_filter_value(value)
            ),
        }
    }
}

async fn rm_group(
    name: String,
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

    if container.is_some() {
        crate::commands::users::validate_base_exists(&mut ldap, &target_base)
            .await
            .with_context(|| format!("The container '{}' could not be validated", target_base))?;
    }

    let lookup = parse_group_lookup(&name)?;

    if let GroupLookup::FullDn(dn) = &lookup
        && !dn_is_within_scope(dn, &target_base)
    {
        bail!(
            "The full DN '{}' is outside the allowed search scope '{}'.",
            dn,
            target_base
        );
    }

    let ldap_filter = lookup.ldap_filter();

    println!("Searching for the group to remove...");
    println!("Base: {}", target_base);
    println!("Input: {}", name);
    println!("Detected input type: {}", lookup.input_type());
    println!("Lookup attribute: {}", lookup.lookup_attribute());
    println!("Lookup value: {}", lookup.original_value());
    println!("Filter: {}\n", ldap_filter);

    let (res, _) = ldap
        .search(
            target_base.as_str(),
            ldap3::Scope::Subtree,
            &ldap_filter,
            vec!["cn", "sAMAccountName"],
        )
        .await?
        .success()?;

    let matches = res
        .into_iter()
        .map(SearchEntry::construct)
        .collect::<Vec<_>>();

    if matches.is_empty() {
        bail!(
            "No group found using {} '{}' under '{}'.",
            lookup.input_type(),
            lookup.original_value(),
            target_base
        );
    }

    if matches.len() > 1 {
        eprintln!(
            "Ambiguous group identifier: {} '{}' matched {} groups:",
            lookup.input_type(),
            lookup.original_value(),
            matches.len()
        );
        for entry in &matches {
            let cn = get_attr(entry, "cn");
            let sam = get_attr(entry, "sAMAccountName");
            eprintln!("  - {} (cn='{}', sAMAccountName='{}')", entry.dn, cn, sam);
        }
        bail!("Refusing to delete because the identifier is ambiguous.");
    }

    let entry = &matches[0];
    println!("Matched exactly one group:");
    println!("  DN: {}", entry.dn);
    println!("  CN: {}", get_attr(entry, "cn"));
    println!("  sAMAccountName: {}", get_attr(entry, "sAMAccountName"));

    if dry_run {
        println!("\nDry run enabled. No changes made.");
        return Ok(());
    }

    if !no_confirm {
        print!("\nAre you sure you want to delete this group? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Operation cancelled.");
            return Ok(());
        }
    }

    println!("\nDeleting group '{}'...", entry.dn);
    let res = ldap
        .delete(entry.dn.as_str())
        .await
        .with_context(|| format!("Failed to delete group '{}'", entry.dn))?;

    res.success()
        .with_context(|| format!("The directory server rejected deletion of '{}'", entry.dn))?;

    println!("Successfully deleted group '{}'", entry.dn);
    Ok(())
}

async fn join_group(
    name: String,
    filter: Option<String>,
    ldap_filter: Option<String>,
    container: Option<DistinguishedName>,
    dry_run: bool,
) -> Result<()> {
    modify_group_members(
        name,
        filter,
        ldap_filter,
        container,
        dry_run,
        MembershipOperation::Join,
    )
    .await
}

async fn leave_group(
    name: String,
    filter: Option<String>,
    ldap_filter: Option<String>,
    container: Option<DistinguishedName>,
    dry_run: bool,
) -> Result<()> {
    modify_group_members(
        name,
        filter,
        ldap_filter,
        container,
        dry_run,
        MembershipOperation::Leave,
    )
    .await
}

async fn modify_group_members(
    name: String,
    filter: Option<String>,
    ldap_filter: Option<String>,
    container: Option<DistinguishedName>,
    dry_run: bool,
    operation: MembershipOperation,
) -> Result<()> {
    if filter.is_none() && ldap_filter.is_none() {
        bail!("You must provide either --filter or --ldap-filter to select users.");
    }

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

    if container.is_some() {
        crate::commands::users::validate_base_exists(&mut ldap, &target_base)
            .await
            .with_context(|| format!("The container '{}' could not be validated", target_base))?;
    }

    let mut group_entry = resolve_single_group(&mut ldap, &target_base, &name).await?;
    let user_filter = build_user_selection_filter(filter, ldap_filter)?;

    println!("Searching users in base: {}", target_base);
    println!("User filter: {}\n", user_filter);

    let (user_res, _) = ldap
        .search(
            target_base.as_str(),
            Scope::Subtree,
            &user_filter,
            vec!["sAMAccountName", "cn"],
        )
        .await?
        .success()?;

    if user_res.is_empty() {
        println!("No users found for the provided filter.");
        return Ok(());
    }

    let users = user_res
        .into_iter()
        .map(SearchEntry::construct)
        .collect::<Vec<_>>();
    let candidate_dns = users
        .iter()
        .map(|entry| entry.dn.clone())
        .collect::<HashSet<_>>();

    let current_members = group_entry
        .attrs
        .remove("member")
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>();

    let to_change = match operation {
        MembershipOperation::Join => candidate_dns
            .iter()
            .filter(|dn| !current_members.contains(*dn))
            .cloned()
            .collect::<Vec<_>>(),
        MembershipOperation::Leave => candidate_dns
            .iter()
            .filter(|dn| current_members.contains(*dn))
            .cloned()
            .collect::<Vec<_>>(),
    };

    println!("Matched group: {}", group_entry.dn);
    println!("Current members: {}", current_members.len());
    println!("Selected users: {}", users.len());

    let action_label = match operation {
        MembershipOperation::Join => "to add",
        MembershipOperation::Leave => "to remove",
    };

    println!("Users {}: {}", action_label, to_change.len());
    for (index, user_dn) in to_change.iter().take(20).enumerate() {
        println!("  {}: {}", index + 1, user_dn);
    }
    if to_change.len() > 20 {
        println!("  ...and {} more.", to_change.len() - 20);
    }

    if to_change.is_empty() {
        println!("No membership changes required.");
        return Ok(());
    }

    if dry_run {
        println!("\nDry run enabled. No changes made.");
        return Ok(());
    }

    let mut success = 0usize;
    let mut failures = 0usize;

    for user_dn in &to_change {
        let op = match operation {
            MembershipOperation::Join => Mod::Add("member", HashSet::from_iter([user_dn.as_str()])),
            MembershipOperation::Leave => {
                Mod::Delete("member", HashSet::from_iter([user_dn.as_str()]))
            }
        };

        match ldap.modify(group_entry.dn.as_str(), vec![op]).await {
            Ok(res) => {
                if res.success().is_ok() {
                    success += 1;
                } else {
                    failures += 1;
                }
            }
            Err(_) => {
                failures += 1;
            }
        }
    }

    let done_label = match operation {
        MembershipOperation::Join => "Added",
        MembershipOperation::Leave => "Removed",
    };

    println!(
        "{} {}/{} users {} group '{}'.",
        done_label,
        success,
        to_change.len(),
        if matches!(operation, MembershipOperation::Join) {
            "to"
        } else {
            "from"
        },
        group_entry.dn
    );

    if failures > 0 {
        println!("Failed operations: {}", failures);
    }

    Ok(())
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

async fn resolve_single_group(
    ldap: &mut Ldap,
    target_base: &DistinguishedName,
    name: &str,
) -> Result<SearchEntry> {
    let lookup = parse_group_lookup(name)?;

    if let GroupLookup::FullDn(dn) = &lookup
        && !dn_is_within_scope(dn, target_base)
    {
        bail!(
            "The full DN '{}' is outside the allowed search scope '{}'.",
            dn,
            target_base
        );
    }

    let ldap_filter = lookup.ldap_filter();

    println!("Searching for target group...");
    println!("Base: {}", target_base);
    println!("Input: {}", name);
    println!("Detected input type: {}", lookup.input_type());
    println!("Lookup attribute: {}", lookup.lookup_attribute());
    println!("Lookup value: {}", lookup.original_value());
    println!("Filter: {}\n", ldap_filter);

    let (res, _) = ldap
        .search(
            target_base.as_str(),
            Scope::Subtree,
            &ldap_filter,
            vec!["cn", "sAMAccountName", "member"],
        )
        .await?
        .success()?;

    let matches = res
        .into_iter()
        .map(SearchEntry::construct)
        .collect::<Vec<_>>();

    if matches.is_empty() {
        bail!(
            "No group found using {} '{}' under '{}'.",
            lookup.input_type(),
            lookup.original_value(),
            target_base
        );
    }

    if matches.len() > 1 {
        eprintln!(
            "Ambiguous group identifier: {} '{}' matched {} groups:",
            lookup.input_type(),
            lookup.original_value(),
            matches.len()
        );
        for entry in &matches {
            let cn = get_attr(entry, "cn");
            let sam = get_attr(entry, "sAMAccountName");
            eprintln!("  - {} (cn='{}', sAMAccountName='{}')", entry.dn, cn, sam);
        }
        bail!("Refusing operation because the identifier is ambiguous.");
    }

    Ok(matches.into_iter().next().expect("group match exists"))
}

fn build_user_selection_filter(
    filter: Option<String>,
    ldap_filter: Option<String>,
) -> Result<String> {
    if let Some(raw) = ldap_filter {
        return Ok(raw);
    }

    if let Some(f) = filter {
        let pattern = if f.contains('*') {
            f
        } else {
            format!("*{}*", f)
        };

        return Ok(format!(
            "(&(objectClass=user)(|(cn={0})(sAMAccountName={0})(mail={0})))",
            pattern
        ));
    }

    bail!("You must provide either --filter or --ldap-filter to select users.")
}

fn parse_group_lookup(input: &str) -> Result<GroupLookup> {
    if input.contains(',') {
        return Ok(GroupLookup::FullDn(DistinguishedName::try_from(input)?));
    }

    if let Some((attr, value)) = input.split_once('=') {
        let attr = attr.trim().to_uppercase();
        let value = value.trim();

        if value.is_empty() {
            bail!(
                "Invalid RDN '{}'. Use a full DN, an RDN like CN=My Group, or a sAMAccountName.",
                input
            );
        }

        return match attr.as_str() {
            "CN" => Ok(GroupLookup::RdnCn(value.to_string())),
            _ => bail!(
                "Unsupported RDN attribute '{}'. Use a full DN, an RDN starting with CN=, or a sAMAccountName.",
                attr
            ),
        };
    }

    Ok(GroupLookup::SamAccountName(input.to_string()))
}

fn escape_ldap_filter_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '*' => escaped.push_str(r"\2a"),
            '(' => escaped.push_str(r"\28"),
            ')' => escaped.push_str(r"\29"),
            '\\' => escaped.push_str(r"\5c"),
            '\0' => escaped.push_str(r"\00"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn dn_is_within_scope(child: &DistinguishedName, scope: &DistinguishedName) -> bool {
    let child_parts = child
        .as_str()
        .split(',')
        .map(|part| part.trim())
        .collect::<Vec<_>>();
    let scope_parts = scope
        .as_str()
        .split(',')
        .map(|part| part.trim())
        .collect::<Vec<_>>();

    if scope_parts.len() > child_parts.len() {
        return false;
    }

    child_parts[child_parts.len() - scope_parts.len()..]
        .iter()
        .zip(scope_parts.iter())
        .all(|(child_part, scope_part)| child_part.eq_ignore_ascii_case(scope_part))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_dn_lookup() {
        let lookup = parse_group_lookup("CN=Grupo Uno,OU=Spain,DC=LAB,DC=INTERNAL").unwrap();
        assert_eq!(
            lookup,
            GroupLookup::FullDn(
                DistinguishedName::try_from("CN=Grupo Uno,OU=Spain,DC=LAB,DC=INTERNAL").unwrap()
            )
        );
    }

    #[test]
    fn parse_rdn_cn_lookup() {
        let lookup = parse_group_lookup("cn=Grupo Ágil").unwrap();
        assert_eq!(lookup, GroupLookup::RdnCn("Grupo Ágil".to_string()));
    }

    #[test]
    fn parse_sam_lookup() {
        let lookup = parse_group_lookup("equipo-研发").unwrap();
        assert_eq!(
            lookup,
            GroupLookup::SamAccountName("equipo-研发".to_string())
        );
    }

    #[test]
    fn reject_unsupported_rdn_attr() {
        assert!(parse_group_lookup("OU=Spain").is_err());
    }

    #[test]
    fn escape_ldap_filter_special_chars() {
        assert_eq!(escape_ldap_filter_value(r"A*(B)\\C"), r"A\2a\28B\29\5c\5cC");
    }

    #[test]
    fn detect_scope_membership() {
        let child = DistinguishedName::try_from("CN=Grupo,OU=Spain,DC=LAB,DC=INTERNAL").unwrap();
        let scope = DistinguishedName::try_from("OU=Spain,DC=LAB,DC=INTERNAL").unwrap();
        assert!(dn_is_within_scope(&child, &scope));
    }

    #[test]
    fn detect_scope_outside_membership() {
        let child = DistinguishedName::try_from("CN=Grupo,OU=France,DC=LAB,DC=INTERNAL").unwrap();
        let scope = DistinguishedName::try_from("OU=Spain,DC=LAB,DC=INTERNAL").unwrap();
        assert!(!dn_is_within_scope(&child, &scope));
    }

    #[test]
    fn build_user_filter_from_simple_filter() {
        let ldap_filter = build_user_selection_filter(Some("qa".to_string()), None).unwrap();
        assert_eq!(
            ldap_filter,
            "(&(objectClass=user)(|(cn=*qa*)(sAMAccountName=*qa*)(mail=*qa*)))"
        );
    }

    #[test]
    fn build_user_filter_from_raw_filter() {
        let ldap_filter =
            build_user_selection_filter(None, Some("(sAMAccountName=test*)".to_string())).unwrap();
        assert_eq!(ldap_filter, "(sAMAccountName=test*)");
    }

    #[test]
    fn reject_empty_user_filter_input() {
        assert!(build_user_selection_filter(None, None).is_err());
    }
}
