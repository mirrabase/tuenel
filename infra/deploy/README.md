# Deployment installer

Run the installer from the repository root:

```sh
./install.sh
```

The interactive wizard offers three modes:

- **Direct:** no domain, public IP, or Traefik is required. The web console is
  published on `0.0.0.0:4050` and the API on `0.0.0.0:4060`. Traffic is plain
  HTTP, so use a firewall or trusted VPN and do not expose this mode directly
  to the internet. Direct mode disables the HTTPS-only session-cookie flag so
  remote clients can sign in over the trusted HTTP network.
- **Bundled Traefik:** uses `compose.production.yaml`, the
  `bundled-traefik` profile, and automatic Let's Encrypt certificates.
- **Existing Traefik:** attaches Tuenel to an existing external Docker network.
  The network must exist before installation.

For automation, pass the mode explicitly:

```sh
./install.sh direct
./install.sh bundled-traefik
./install.sh existing-traefik
```

On first use, direct mode creates `.env`; Traefik modes create
`.env.production`. Generated files use permission mode `0600`, contain random
secrets, are ignored by Git, and are never overwritten. Interactive first-time
setup prints a one-time browser link; only its token hash is stored and the
account password is entered in the browser. Production setup requires a pinned
release tag.

Direct mode defaults to standalone, invite-only authentication with manual
invitation links. The production wizard can instead select a shared managed
deployment and public, invite-only, or closed registration. Public signup and
email invitations require a Resend API key and verified sender.

After installation, use the same Compose file and env file for operations:

```sh
# Direct
docker compose --env-file .env -f compose.yaml ps
docker compose --env-file .env -f compose.yaml logs -f gateway gateway-web
docker compose --env-file .env -f compose.yaml down

# Bundled Traefik
docker compose --env-file .env.production -f compose.production.yaml --profile bundled-traefik ps

# Existing Traefik
docker compose --env-file .env.production -f compose.production.yaml ps
```
