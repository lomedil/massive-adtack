use std::collections::HashMap;
use std::sync::OnceLock;

static OID_MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();

pub fn get_oid_name(oid: &str) -> Option<String> {
    let map = OID_MAP.get_or_init(|| {
        let content = include_str!("oids.txt");
        let mut m = HashMap::new();
        for line in content.lines() {
            if let Some((k, v)) = line.split_once(':') {
                m.insert(k.trim(), v.trim());
            }
        }
        m
    });

    map.get(oid).map(|s| s.to_string())
}
