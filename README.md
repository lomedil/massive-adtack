# 🎸 Project: Massive AD-tack (`mad`)

**Massive AD-tack** is a high-performance LDAP stress-testing and provisioning tool written in **Rust**. It is designed to "attack" (saturate with data) Active Directory environments to validate synchronization services, performance bottlenecks, and schema limits.

## 🚀 The Binary: `mad`
The command-line interface is optimized for speed. `mad` allows for the rapid creation of complex directory structures, thousands of users, and nested groups.

---

## 🛠️ Technical Context

### Target Environment
* **Primary Target:** Samba 4 acting as an **Active Directory Domain Controller (AD DC)**.
* **Infrastructure:** Deployed via Docker on **WSL2** (using the Linux native file system for `xattr` support).
* **Connection:** LDAP v3 via `localhost:10389` (default remapped port to avoid Windows port 445/389 conflicts).

### LDAP & AD Requirements
To successfully "attack" an AD/Samba instance, the tool implements:
1.  **AD-Specific Schema:** Generation of `sAMAccountName`, `userPrincipalName`, and `unicodePwd` (requires LDAPS/TLS for password setting).
2.  **Paging Support:** Mandatory use of the **Simple Paged Results Control** (OID `1.2.840.113556.1.4.319`) for reading back large datasets.
3.  **Root DSE Discovery:** Auto-interrogation of the server (DN "") to verify capabilities (`supportedControl`, `supportedLDAPVersion`) before starting execution.

---

## 🏗️ Architecture Goals (for AI Agents)

When generating code for **Massive AD-tack**, follow these principles:

* **Asynchronous I/O:** Use `tokio` and `ldap3` in its async flavor to handle multiple concurrent Binds and Add operations.
* **Deterministic Data:** Use seeds for user generation so that "Attacks" can be reproducible across different environments.
* **Performance:** Minimize allocations in the hot path of user creation. Rust's ownership model should be used to reuse buffers for LDAP attributes.
* **Error Handling:** Distinguish between "Server Saturated" (Timeout) and "Schema Violation" (Constraint Violation).

---

## 📋 Quick Reference for Agents
* **Default Base DN:** `DC=lab,DC=local`
* **Default Admin:** `CN=Administrator,CN=Users,DC=lab,DC=local`
* **Mandatory User Classes:** `top`, `person`, `organizationalPerson`, `user`.
* **Naming Convention:** All generated objects should ideally follow a pattern (e.g., `mad-user-0001`) for easy cleanup.

---

> **Note:** This tool is strictly for internal testing and performance benchmarking. It assumes full administrative access to the target LDAP/Samba server.