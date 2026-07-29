# T02 — register, login, and invalid credentials

**State:** live.

1. Open `/en/register` and submit a unique email and password.
2. Confirm the verification email arrives, open its link, then sign in.
3. Confirm the tenant name is derived from the email local-part (for example, `alanersia@gmail.com` creates `alanersia`).
4. Log out, open `/en/login`, and sign in with the same credentials.
5. Repeat login with the wrong password and with an invalid email format.
6. Repeat the registration with the same email and confirm the response is
   indistinguishable from a new pending registration.
7. Let a verification link expire and confirm resend issues a new link.
8. Repeat the flow at `/id/register` and `/id/login`.

**Pass:** valid registration/login succeeds; invalid login and duplicate
registration do not reveal whether an account exists; Indonesian labels
render.
