@~/.claude/prompts/new_functionality_prompt_spec.md

# Deploy nginx:alpine Web Server Behind Traefik on GCP VM

## Role
Act as a Software Developer, Software Architect and IT Infrastructure Engineer, you are an expert in Docker, Traefik v3, nginx, and Google Cloud VM deployments.

## Context

### Production Infrastructure (GCP VM)
- **VM IP:** `34.174.56.186`
- **SSH:** `ssh -i C:\ubuntuiso\.ssh\vboxuser gcvmuser@34.174.56.186`
- **Traefik version:** v3.3 (already running)
- **Docker network:** `miseia-net` (external bridge — all services must join it)
- **TLS cert resolver:** `cloudflare` (wildcard `*.deviaaps.com` via Cloudflare DNS-01)
- **Domain:** `deviaaps.com`
- **Target URL for this service:** `https://letsencrypt-client.deviaaps.com`
- **Reference compose file:** `D:\Master-IA-Dev\00-GoogleCloud\004_Infra_in_VM\docker-compose.yml`
- **Reference .env:** `D:\Master-IA-Dev\00-GoogleCloud\004_Infra_in_VM\.env`

### This Project
- **Project root (local):** `D:\Master-IA-Dev\06-Bloque6\1-6-30-letsencrypt-client`
- **Project root (VM):** `~/MISEIA_1-6-30-letsencrypt-client`
- **GitHub remote:** `https://github.com/Jorgeaapaz/MISEIA_1-6-30-letsencrypt-client`
- **GitLab remote:** `https://gitlab.codecrypto.academy/jorgeaapaz/MISEIA_1-6-30-letsencrypt-client`
- **Static file to serve:** `index.html` (at project root)

## Task

Create a production `docker-compose.prod.yml` in the project root that:

1. Defines an `nginx:alpine` service that serves `index.html`.
2. Joins the existing `miseia-net` external Docker network.
3. Carries Traefik labels so Traefik automatically routes `https://letsencrypt-client.deviaaps.com` → nginx port 80, with TLS via the `cloudflare` cert resolver.
4. Does **not** expose any ports directly (Traefik is the only ingress).
5. Deploys the file to the VM and starts the service.

### Feature Guidelines

#### Files to create locally

**`docker-compose.prod.yml`** (project root):
```yaml
networks:
  miseia-net:
    external: true

services:
  letsencrypt-client-web:
    image: nginx:alpine
    container_name: letsencrypt-client-web
    restart: unless-stopped
    volumes:
      - ./index.html:/usr/share/nginx/html/index.html:ro
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.letsencrypt-client.rule=Host(`letsencrypt-client.deviaaps.com`)"
      - "traefik.http.routers.letsencrypt-client.entrypoints=websecure"
      - "traefik.http.routers.letsencrypt-client.tls=true"
      - "traefik.http.routers.letsencrypt-client.tls.certresolver=cloudflare"
      - "traefik.http.services.letsencrypt-client-svc.loadbalancer.server.port=80"
    networks:
      - miseia-net
```

> No custom `nginx.conf` is needed — the default nginx config already serves files from `/usr/share/nginx/html/`.

#### Deployment steps (VM)

All remote commands run via:
```
ssh -i C:\ubuntuiso\.ssh\vboxuser gcvmuser@34.174.56.186
```

1. Create the project directory on the VM if it does not exist:
   ```bash
   mkdir -p ~/MISEIA_1-6-30-letsencrypt-client
   ```
2. Copy `docker-compose.prod.yml` and `index.html` to the VM:
   ```powershell
   scp -i C:\ubuntuiso\.ssh\vboxuser `
     D:\Master-IA-Dev\06-Bloque6\1-6-30-letsencrypt-client\docker-compose.prod.yml `
     D:\Master-IA-Dev\06-Bloque6\1-6-30-letsencrypt-client\index.html `
     gcvmuser@34.174.56.186:~/MISEIA_1-6-30-letsencrypt-client/
   ```
3. Start the service on the VM:
   ```bash
   cd ~/MISEIA_1-6-30-letsencrypt-client
   docker compose -f docker-compose.prod.yml up -d
   ```
4. Verify the container is running and Traefik picked it up:
   ```bash
   docker ps --filter name=letsencrypt-client-web
   docker logs letsencrypt-client-web
   ```
5. Test the HTTPS endpoint from local machine:
   ```powershell
   curl https://letsencrypt-client.deviaaps.com
   ```

## Output format

Two files saved to the project:
1. `docker-compose.prod.yml` — at project root
2. This prompt file is already at `docs/prompts/feature_002_nginx_traefik_deploy_prompt.md`

## Examples and Steps to follow

1. **Git**: Create local branch `feature/002-nginx-traefik-deploy`.
2. **Create** `docker-compose.prod.yml` at project root with the content above.
3. **Commit** locally: `git commit -m "feat: add nginx:alpine + Traefik production compose"`.
4. **Push** feature branch and open PR via `/git-only-update`.
5. **Merge** PR into main and pull locally.
6. **Deploy** to VM using `scp` + `docker compose -f docker-compose.prod.yml up -d` as described above.
7. **Verify** with `curl https://letsencrypt-client.deviaaps.com` — should return the `index.html` content.
8. **Confirm** TLS is valid (no certificate warnings; issued by Let's Encrypt via Cloudflare DNS-01).

## Output checklist and Guardrails
- [ ] `docker-compose.prod.yml` exists at project root
- [ ] Service image is `nginx:alpine`
- [ ] `index.html` is mounted read-only at `/usr/share/nginx/html/index.html`
- [ ] No ports are published directly on the host
- [ ] Service joins `miseia-net` as an external network
- [ ] Traefik labels present: `enable=true`, `Host(...)` rule, `websecure` entrypoint, `tls=true`, `certresolver=cloudflare`
- [ ] `restart: unless-stopped` set
- [ ] `https://letsencrypt-client.deviaaps.com` returns HTTP 200
- [ ] TLS certificate is valid (Let's Encrypt, not self-signed)
- [ ] HTTP → HTTPS redirect works (Traefik global redirect already handles this)
- [ ] Committed, PR merged, and deployed before marking done
