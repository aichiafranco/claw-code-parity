//! Hex UUIDs matching pyJianYingDraft (`uuid4().hex`).

use uuid::Uuid;

#[must_use]
pub fn hex_id() -> String {
    Uuid::new_v4().simple().to_string()
}

#[must_use]
pub fn upper_guid() -> String {
    Uuid::new_v4().to_string().to_uppercase()
}
