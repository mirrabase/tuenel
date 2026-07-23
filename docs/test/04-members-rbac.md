# T05 — members, invitation, and RBAC

**State:** live.

1. As the owner, open **Members** and invite a second registered user as `engineer`.
2. Copy the one-time invite link/token shown by the UI.
3. Accept it while signed in as the invited user.
4. Switch to the invited tenant and confirm the membership role is shown.
5. Repeat with `viewer`; confirm viewer cannot submit inference or admin mutations.
6. Confirm a viewer cannot invite members and an engineer cannot invite members.
7. Try inviting an owner role.

**Pass:** owner/admin invitation works, the invite is tenant-bound and one-time, viewer/engineer permissions are enforced, and owner-role invitations are rejected.
