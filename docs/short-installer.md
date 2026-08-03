# Short installer URL

The public repository includes a remote bootstrap at
`install-bootstrap.sh`. It downloads a pinned release archive and then runs
the same `install.sh` used by a local checkout.

The intended command is:

```sh
curl -fsSL https://install.tuenel.com | sh
```

## Cloudflare redirect

No VM or package installation is required for the short URL. In the Tuenel
Cloudflare zone:

1. Create a proxied DNS record for `install` (an arbitrary placeholder target
   is sufficient for a redirect rule).
2. Create a Redirect Rule matching `install.tuenel.com/*`.
3. Redirect to:
   `https://raw.githubusercontent.com/mirrabase/tuenel/main/install-bootstrap.sh`
4. Use a permanent (`301`) redirect and keep the path/query forwarding off.

`curl -fsSL` follows the redirect and executes the bootstrap script. The
bootstrap defaults to the latest validated stable release reference in the
script; override it explicitly with `TUENEL_VERSION=X.Y.Z` when required.

For a local checkout, continue using `./install.sh`. The Cloudflare redirect
does not expose secrets or require a process running on the deployment VM.
