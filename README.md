# 🎸 Massive AD-tack (`mad`)

**Massive AD-tack** is a high-performance LDAP stress-testing and provisioning tool written in **Rust**. It is designed to "attack" (fill with data) Active Directory environments to validate synchronization services, performance bottlenecks, and schema limits.

> [!NOTE]
> **🧪 Experiment & Philosophy**
>
> This project is an experiment in **AI-Assisted Development** using **Antigravity**.
>
> It serves a dual purpose:
> 1.  To build a robust tool for real-world scenarios.
> 2.  To explore how AI agents can boost productivity **without replacing the joy of programming**. It is about allowing the human to focus on the *craft* while the agent handles the *heavy lifting*.

## 🚀 Features

-   **Blazingly Fast**: Built with Rust, `tokio`, and `ldap3` for asynchronous performance.
-   **Bulk User Creation**: Generate thousands of users with customizable naming patterns.
-   **Group Information**: List groups with member counts and CN details.
-   **Smart Cleanup**: Search-based deletion with safety checks (dry-run, confirmation prompts).
-   **Connectivity Checks**: Verify server capabilities and supported controls (e.g., Paged Results).

## 📦 Installation

To build and install from source, ensure you have a standard Rust toolchain installed.

```bash
git clone https://github.com/lomedil/massive-adtack.git
cd massive-adtack
cargo install --path .
```

## ⚙️ Configuration

`mad` looks for a configuration file in the following order:
1.  Environment variable `MAD_CONFIG`
2.  `.agents/config.toml` (for agent use)
3.  `config.toml` (in the current directory)

### Example `config.toml`

```toml
url = "ldap://localhost:10389"
base_dn = "DC=lab,DC=local"
user = "CN=Administrator,CN=Users,DC=lab,DC=local"
password = "StrongPassword123!"
starttls = false
tls_ca_cert = "never" # or path to CA cert
username_format = "{first_name}.{last_name}{counter}"

[mappings]
username = "sAMAccountName"
first_name = "givenName"
last_name = "sn"
email = "mail"
phone = "telephoneNumber"
```

### 1. Check Connectivity
Verify that `mad` can reach your target Active Directory environment.

```bash
mad check
# Output in JSON format
mad check --json
```

### 2. Bulk Create Users
Create 1,000 users with a specific naming format.

```bash
mad users add --count 1000 --format "test_user_{counter}"
```

### 3. List Users
Search for users using simple wildcards or raw LDAP filters.

```bash
# Verify creation
mad users list --filter "test_user_*"

# Advanced LDAP filter
mad users list --ldap-filter "(&(objectCategory=person)(objectClass=user)(sAMAccountName=test_*))"
```

### 4. Housekeeping (Delete Users)
Clean up users matching a pattern. **Always dry-run first!**

```bash
# 1. Dry run to see what would be deleted
mad users rm "test_user_*" --dry-run

# 2. Perform deletion (prompts for confirmation)
mad users rm "test_user_*"

# 3. Force deletion (no confirmation)
mad users rm "test_user_*" --no-confirm
```

### 5. List Groups
Search for groups and see member counts.

```bash
mad groups list --filter "Sync"
```

## ⚙️ Technical Context

### Target Environment
*   **Primary Target:** Samba 4 acting as an **Active Directory Domain Controller (AD DC)**.
*   **Protocol:** LDAP v3 (default port `10389` to avoid conflicts on dev machines).

### Key Constraints
To successfully operate against AD/Samba, `mad` handles:
1.  **AD Schemas:** `sAMAccountName`, `userPrincipalName`, `unicodePwd`.
2.  **Paging:** Uses **Simple Paged Results** (OID `1.2.840.113556.1.4.319`) for large result sets.
3.  **Root DSE:** Auto-discovery of server capabilities.

## 🧪 Test Lab

A Docker-based test environment (Samba AD DC) is available in the `testlab/` directory.
Check out [testlab/how-to-deploy.md](testlab/how-to-deploy.md) for deployment instructions.

## 🤖 For AI Agents

If you are an AI agent (Opencode, Antigravity, etc.) tasked with maintaining this codebase, please refer to [AGENTS.md](./AGENTS.md) for architectural guidelines and coding standards.

## 📄 License

This project is licensed under the [MIT License](LICENSE).