//! Typed identifiers.
//!
//! Upstream mixes integers and strings for the same field (for example
//! `courseId` may arrive as `9222783` or `"9222783"`). Every id is therefore
//! normalised to a string on construction so equality and cache keys stay
//! stable.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! string_id {
    ($name:ident, $label:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl fmt::Display) -> Self {
                Self(value.to_string())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<i64> for $name {
            fn from(value: i64) -> Self {
                Self(value.to_string())
            }
        }

        impl $name {
            pub const LABEL: &'static str = $label;
        }
    };
}

string_id!(AccountId, "account");
string_id!(CourseId, "course");
string_id!(JobId, "job");
string_id!(BookId, "book");
string_id!(QuestionId, "question");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_normalise_numbers_and_strings_to_the_same_value() {
        assert_eq!(CourseId::from(9222783), CourseId::from("9222783"));
        assert_eq!(JobId::new("JOB-1").as_str(), "JOB-1");
    }

    #[test]
    fn ids_serialise_transparently() {
        let json = serde_json::to_string(&BookId::from("BOOK-1")).unwrap();
        assert_eq!(json, "\"BOOK-1\"");
    }
}
