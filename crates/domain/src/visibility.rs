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

/// A wishlist's kind (SPEC §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WishKind {
    /// Someone's own wishlist.
    Own,
    /// Gift ideas for someone else (the recipient).
    Gift,
}

/// The user's role on a wishlist, or `None`. Like lists, except that the recipient of a gift
/// list never sees it, whatever it is shared with (SPEC §6.9, §7.3).
pub fn wishlist_role(
    kind: WishKind,
    is_owner: bool,
    is_recipient: bool,
    shares: impl IntoIterator<Item = Role>,
) -> Option<Role> {
    if kind == WishKind::Gift && is_recipient {
        return None;
    }
    list_role(is_owner, shares)
}

/// Whether someone who can see a wishlist sees its claims ("I'll get this"): everyone but the
/// owner of their own wishlist, so surprises stay surprises.
pub fn sees_claims(kind: WishKind, is_owner: bool) -> bool {
    !(kind == WishKind::Own && is_owner)
}

/// Who may claim an item: whoever sees the claims (viewers too: claiming is what sharing a
/// wishlist is for).
pub fn may_claim(kind: WishKind, is_owner: bool) -> bool {
    sees_claims(kind, is_owner)
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

    #[test]
    fn gift_lists_never_reach_their_recipient() {
        use WishKind::*;
        // The recipient is excluded even when the list is shared with them (e.g. through a
        // household) and even as an editor.
        assert_eq!(wishlist_role(Gift, false, true, [Role::Editor]), None);
        assert_eq!(
            wishlist_role(Gift, false, false, [Role::Viewer]),
            Some(Role::Viewer)
        );
        assert_eq!(wishlist_role(Gift, true, false, []), Some(Role::Owner));
        // Own wishlists have no recipient.
        assert_eq!(
            wishlist_role(Own, false, true, [Role::Viewer]),
            Some(Role::Viewer)
        );
        assert_eq!(wishlist_role(Own, false, false, []), None);
    }

    #[test]
    fn claims_stay_surprises() {
        use WishKind::*;
        assert!(!sees_claims(Own, true));
        assert!(sees_claims(Own, false));
        assert!(sees_claims(Gift, true));
        assert!(sees_claims(Gift, false));
        assert!(!may_claim(Own, true));
        assert!(may_claim(Gift, true));
    }
}
