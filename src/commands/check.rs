use crate::config::Config;
use crate::oids::get_oid_name;
use anyhow::{Context, Result};
use ldap3::{LdapConnAsync, LdapConnSettings, Scope, SearchEntry};
use serde::Serialize;

#[derive(Serialize)]
pub struct ControlInfo {
    pub oid: String,
    pub name: Option<String>,
}

#[derive(Serialize)]
pub struct CheckResult {
    pub url: String,
    pub bound_as: String,
    pub vendor: Option<String>,
    pub dns_host_name: Option<String>,
    pub is_samba: bool,
    pub supported_ldap_versions: Vec<String>,
    pub has_paging_support: bool,
    pub supported_controls: Vec<ControlInfo>,
    pub supported_extensions: Vec<String>,
}

pub async fn execute(json: bool) -> Result<()> {
    let cfg = Config::load()?;
    if !json {
        println!("Checking connectivity to: {}", cfg.url);
    }

    let mut settings = LdapConnSettings::new();
    if let Some(ca_cert) = &cfg.tls_ca_cert {
        if ca_cert == "never" {
            if !json {
                println!("Note: tls_ca_cert is 'never'. Disabling certificate verification...");
            }
            settings = settings.set_no_tls_verify(true);
        } else if !json {
            println!("Note: Using CA cert from: {}", ca_cert);
        }
    }

    let (conn, mut ldap) = LdapConnAsync::with_settings(settings, &cfg.url)
        .await
        .context("Failed to connect to LDAP server")?;

    ldap3::drive!(conn);

    // StartTLS check note
    if !json && (cfg.starttls || cfg.url.starts_with("ldap://")) {
        println!(
            "Note: If the server requires encryption (rc=8 strongerAuthRequired), please use an 'ldaps://' URL in your config."
        );
    }

    if !json {
        println!("Attempting Simple Bind as: {}", cfg.user);
    }
    ldap.simple_bind(&cfg.user, &cfg.password)
        .await
        .context("Failed to bind to LDAP server")?
        .success()
        .context("LDAP Bind operation failed")?;

    if !json {
        println!("Successfully bound! Interrogating Root DSE...");
    }

    // Root DSE interrogation
    let (entries, _res) = ldap
        .search(
            "",
            Scope::Base,
            "(objectClass=*)",
            vec![
                "supportedControl",
                "dnsHostName",
                "vendorName",
                "supportedLDAPVersion",
                "supportedExtension",
            ],
        )
        .await
        .context("Failed to search Root DSE")?
        .success()
        .context("Search result error")?;

    let mut check_out = CheckResult {
        url: cfg.url.clone(),
        bound_as: cfg.user.clone(),
        vendor: None,
        dns_host_name: None,
        is_samba: false,
        supported_ldap_versions: Vec::new(),
        has_paging_support: false,
        supported_controls: Vec::new(),
        supported_extensions: Vec::new(),
    };

    if let Some(entry_data) = entries.first() {
        let entry = SearchEntry::construct(entry_data.clone());

        check_out.vendor = entry
            .attrs
            .get("vendorName")
            .and_then(|v| v.first().cloned());
        check_out.dns_host_name = entry
            .attrs
            .get("dnsHostName")
            .and_then(|h| h.first().cloned());

        check_out.supported_ldap_versions = entry
            .attrs
            .get("supportedLDAPVersion")
            .cloned()
            .unwrap_or_default();

        check_out.supported_controls = entry
            .attrs
            .get("supportedControl")
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|oid| ControlInfo {
                name: get_oid_name(&oid),
                oid,
            })
            .collect();

        check_out.supported_extensions = entry
            .attrs
            .get("supportedExtension")
            .cloned()
            .unwrap_or_default();

        // Check for Paged Results Control (1.2.840.113556.1.4.319)
        check_out.has_paging_support = check_out
            .supported_controls
            .iter()
            .any(|c| c.oid == "1.2.840.113556.1.4.319");

        let is_samba_vendor = check_out
            .vendor
            .as_ref()
            .map(|v| v.to_lowercase().contains("samba"))
            .unwrap_or_default();

        let is_samba_oid = check_out
            .supported_controls
            .iter()
            .any(|c| c.oid.contains("1.3.6.1.4.1.7165"));

        check_out.is_samba = is_samba_vendor || is_samba_oid;

        if !json {
            println!("--- Server Information ---");
            if let Some(vendor) = &check_out.vendor {
                println!("Vendor: {}", vendor);
            }
            if let Some(host) = &check_out.dns_host_name {
                println!("DNS Host Name: {}", host);
            }
            println!(
                "LDAP Versions: {}",
                check_out.supported_ldap_versions.join(", ")
            );

            if check_out.is_samba {
                println!("Identification: Samba 4 AD DC detected!");
            } else {
                println!("Identification: Generic AD or LDAP server.");
            }

            if check_out.has_paging_support {
                println!("Paging Support: Yes (OID 1.2.840.113556.1.4.319 found)");
            } else {
                println!("Paging Support: No (Critical for bulk operations!)");
            }

            println!("\nSupported Controls:");
            for ctrl in &check_out.supported_controls {
                match &ctrl.name {
                    Some(name) => println!("  - {} ({})", name, ctrl.oid),
                    None => println!("  - {}", ctrl.oid),
                }
            }
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&check_out)?);
    } else {
        ldap.unbind().await.context("Failed to unbind")?;
        println!("\nConnectivity check completed successfully.");
    }

    Ok(())
}
