use chrono::{DateTime,
             Utc};
use serde::{Deserialize,
            Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Contact {
    pub id: Uuid,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub memo: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Contact {
    pub fn new(name: String, email: Option<String>, phone: Option<String>, memo: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.trim().to_string(),
            email: Self::normalize_optional_field(email),
            phone: Self::normalize_optional_field(phone),
            memo: Self::normalize_optional_field(memo),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn update(&mut self, name: Option<String>, email: Option<String>, phone: Option<String>, memo: Option<String>) {
        if let Some(name) = name {
            self.name = name.trim().to_string();
        }
        if let Some(email) = email {
            self.email = Self::normalize_optional_field(Some(email));
        }
        if let Some(phone) = phone {
            self.phone = Self::normalize_optional_field(Some(phone));
        }
        if let Some(memo) = memo {
            self.memo = Self::normalize_optional_field(Some(memo));
        }
        self.updated_at = Utc::now();
    }

    fn normalize_optional_field(value: Option<String>) -> Option<String> { value.map(|value| value.trim().to_string()).filter(|value| !value.is_empty()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_trims_name_and_discards_blank_optional_fields() {
        let contact = Contact::new(
            "  Ada Lovelace  ".to_string(),
            Some("  ".to_string()),
            Some("  +82 10-1234-5678  ".to_string()),
            None,
        );

        assert_eq!(contact.name, "Ada Lovelace");
        assert_eq!(contact.email, None);
        assert_eq!(contact.phone, Some("+82 10-1234-5678".to_string()));
        assert_eq!(contact.memo, None);
    }

    #[test]
    fn update_can_clear_optional_fields_with_blank_values() {
        let mut contact = Contact::new(
            "Ada Lovelace".to_string(),
            Some("ada@example.com".to_string()),
            Some("+82 10-1234-5678".to_string()),
            Some("Seoul".to_string()),
        );

        contact.update(None, Some(" ".to_string()), Some("".to_string()), Some("  Busan  ".to_string()));

        assert_eq!(contact.email, None);
        assert_eq!(contact.phone, None);
        assert_eq!(contact.memo, Some("Busan".to_string()));
    }
}
