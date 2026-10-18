//! Who may do what with a shopping list (SPEC §6.8, §7.3, D-8). The server's `visibility`
//! module finds a user's relation to a list; these rules decide what that relation allows.

/// A user's relation to a list. Anyone without one doesn't see the list at all (404).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Viewer,
    Editor,
    Owner,
}

/// What a request wants to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// See the list, its categories and items.
    Read,
    /// Add, tick, edit and remove items; manage categories.
    EditItems,
    /// Rename, archive, delete, share, make default.
    Manage,
}

pub fn allows(role: Role, need: Need) -> bool {
    match need {
        Need::Read => true,
        Need::EditItems => role >= Role::Editor,
        Need::Manage => role == Role::Owner,
    }
}

/// The strongest relation: owning the list, or the best of the shares that reach the user
/// (directly or through a household).
pub fn list_role(is_owner: bool, shares: impl IntoIterator<Item = Role>) -> Option<Role> {
    if is_owner {
        Some(Role::Owner)
    } else {
        shares.into_iter().max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules() {
        assert!(allows(Role::Viewer, Need::Read));
        assert!(!allows(Role::Viewer, Need::EditItems));
        assert!(allows(Role::Editor, Need::EditItems));
        assert!(!allows(Role::Editor, Need::Manage));
        assert!(allows(Role::Owner, Need::Manage));
    }

    #[test]
    fn roles() {
        assert_eq!(list_role(true, []), Some(Role::Owner));
        assert_eq!(list_role(false, []), None);
        assert_eq!(
            list_role(false, [Role::Viewer, Role::Editor]),
            Some(Role::Editor)
        );
        assert_eq!(list_role(true, [Role::Viewer]), Some(Role::Owner));
    }
}
