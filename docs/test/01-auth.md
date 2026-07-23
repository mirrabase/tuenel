# T02 — register, login, and invalid credentials

**State:** live.

1. Open `/en/register` and submit a unique email, password, and tenant name.
2. Confirm redirect to a URL shaped like `/en/<tenant-uuid>` and the signed-in console appears.
3. Log out, open `/en/login`, and sign in with the same credentials.
4. Repeat login with the wrong password and with an invalid email format.
5. Repeat the registration with the same email.
6. Repeat the flow at `/id/register` and `/id/login`.

**Pass:** valid registration/login succeeds; invalid login is generic and does not reveal whether an account exists; duplicate registration is rejected; Indonesian labels render.
