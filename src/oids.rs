const OIDS: &[(&str, &str)] = &[
    ("1.2.840.113556.1.4.319", "Simple Paged Results"),
    ("1.2.840.113556.1.4.801", "Show Deleted Objects"),
    ("1.2.840.113556.1.4.473", "Server Side Sort"),
    ("1.2.840.113556.1.4.805", "Tree Delete"),
    ("1.2.840.113556.1.4.1338", "Verify Name"),
    ("1.2.840.113556.1.4.1339", "Domain Scope"),
    ("1.2.840.113556.1.4.1340", "Search Options"),
    ("1.2.840.113556.1.4.1413", "Permissive Modify"),
    ("1.2.840.113556.1.4.1504", "ASQ (Attribute Scoped Query)"),
    ("1.2.840.113556.1.4.1852", "DirSync"),
    ("1.2.840.113556.1.4.1943", "Index Hint"),
    ("2.16.840.1.113730.3.4.2", "Manage DSA IT"),
    ("2.16.840.1.113730.3.4.9", "VLV (Virtual List View)"),
    ("1.2.840.113556.1.4.528", "Notification"),
    ("1.2.840.113556.1.4.529", "Extended DN"),
    ("1.2.840.113556.1.4.417", "Show Deactivated Link"),
    ("1.2.840.113556.1.4.2064", "Show Recycled Objects"),
    ("1.2.840.113556.1.4.1341", "RODC Promotional"),
    ("1.3.6.1.4.1.7165.4.3.14", "Samba 4 Policy"),
];

pub fn get_oid_name(oid: &str) -> Option<String> {
    OIDS.iter()
        .find(|(o, _)| *o == oid)
        .map(|(_, name)| name.to_string())
}
