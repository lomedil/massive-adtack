pub struct NamingFormatter {
    template: String,
}

impl NamingFormatter {
    pub fn new(template: Option<String>) -> Self {
        Self {
            template: template.unwrap_or_else(|| "{first_name}.{last_name}".to_string()),
        }
    }

    /// Normalizes a string by converting accented characters and 'ñ' to their ASCII equivalents.
    fn normalize(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                'á' | 'Á' => 'a',
                'é' | 'É' => 'e',
                'í' | 'Í' => 'i',
                'ó' | 'Ó' => 'o',
                'ú' | 'Ú' | 'ü' | 'Ü' => 'u',
                'ñ' | 'Ñ' => 'n',
                _ => c,
            })
            .collect()
    }

    pub fn generate(&self, first_name: &str, last_name: &str, counter: u32) -> String {
        let norm_first = Self::normalize(first_name);
        let norm_last = Self::normalize(last_name);

        let first_name_lower = norm_first.to_lowercase();
        let first_name_upper = norm_first.to_uppercase();
        let last_name_lower = norm_last.to_lowercase();
        let last_name_upper = norm_last.to_uppercase();

        let first_initial_lower = first_name_lower.chars().next().unwrap_or(' ').to_string();
        let first_initial_upper = first_name_upper.chars().next().unwrap_or(' ').to_string();
        let last_initial_lower = last_name_lower.chars().next().unwrap_or(' ').to_string();
        let last_initial_upper = last_name_upper.chars().next().unwrap_or(' ').to_string();

        let counter_str = format!("{:03}", counter);

        let mut result = self.template.clone();

        // Placeholders mapping
        let replacements = [
            ("{first_name}", &first_name_lower),
            ("{FIRST_NAME}", &first_name_upper),
            ("{last_name}", &last_name_lower),
            ("{LAST_NAME}", &last_name_upper),
            ("{first_name_initial}", &first_initial_lower),
            ("{FIRST_NAME_INITIAL}", &first_initial_upper),
            ("{last_name_initial}", &last_initial_lower),
            ("{LAST_NAME_INITIAL}", &last_initial_upper),
            ("{counter}", &counter_str),
        ];

        for (placeholder, value) in replacements {
            result = result.replace(placeholder, value);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_format() {
        let generator = NamingFormatter::new(None);
        assert_eq!(generator.generate("John", "Romero", 1), "john.romero");
    }

    #[test]
    fn test_custom_format_with_initials_and_counter() {
        let generator = NamingFormatter::new(Some(
            "{first_name}.{last_name}__{first_name_initial}{counter}".to_string(),
        ));
        assert_eq!(generator.generate("John", "Romero", 1), "john.romero__j001");
    }

    #[test]
    fn test_case_sensitivity() {
        let generator = NamingFormatter::new(Some(
            "{FIRST_NAME}.{LAST_NAME_INITIAL}{counter}".to_string(),
        ));
        assert_eq!(generator.generate("John", "Romero", 42), "JOHN.R042");
    }

    #[test]
    fn test_normalization() {
        let generator = NamingFormatter::new(Some("{first_name}.{last_name}".to_string()));
        assert_eq!(generator.generate("Adrián", "Peña", 1), "adrian.pena");
    }

    #[test]
    fn test_all_caps_normalization() {
        let generator = NamingFormatter::new(Some("{FIRST_NAME}".to_string()));
        assert_eq!(generator.generate("Adrián", "Peña", 1), "ADRIAN");
    }
}
