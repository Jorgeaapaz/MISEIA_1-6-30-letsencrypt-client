# Retrospectiva de Sesión — 2026-05-02
### Implementación de un cliente ACME en Rust con Pebble (Let's Encrypt local)

---

## Resumen / Overview

Se implementó desde cero un cliente ACME (RFC 8555) completo en Rust que obtiene certificados TLS de forma automatizada. El cliente implementa el flujo completo: cuenta ACME, órdenes, HTTP-01 challenge, CSR, finalización y descarga del certificado. Se probó contra Pebble (servidor ACME de pruebas de Let's Encrypt corriendo en Docker). La sesión fue exitosa: se emitieron certificados para dos dominios distintos, se verificaron con OpenSSL, y se probaron con una app Express HTTPS usando `curl`.

---

## Software instalado / Installation

### Rust (via rustup)

Rust **no estaba instalado** en el sistema. Se instaló descargando `rustup-init.exe` directamente:

```bash
curl -sL "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe" \
  -o /tmp/rustup-init.exe
chmod +x /tmp/rustup-init.exe
/tmp/rustup-init.exe -y --default-host x86_64-pc-windows-msvc --default-toolchain stable
```

**Versión instalada:** `rustc 1.95.0 (59807616e 2026-04-14)` / `cargo 1.95.0`

> **Nota:** En cada sesión nueva de Git Bash hay que exportar el PATH:
> ```bash
> export PATH="$PATH:/c/Users/jorge/.cargo/bin"
> ```
> O añadirlo permanentemente al `~/.bashrc`.

### Imágenes Docker descargadas

```bash
docker pull ghcr.io/letsencrypt/pebble:latest
docker pull ghcr.io/letsencrypt/pebble-challtestsrv:latest
```

> ⚠️ Las imágenes están en **GitHub Container Registry** (`ghcr.io`), NO en Docker Hub. El nombre `letsencrypt/pebble` en Docker Hub no existe.

### Node.js / npm (ya estaba instalado)

- Node: `v24.14.0`
- Dependencias del test-app: `cd test-app && npm install` (solo `express`)

---

## Archivos creados / Files Created

```
letsencrypt-client/
├── CLAUDE.md                        # Especificaciones del proyecto (refinadas)
├── Cargo.toml                       # Dependencias Rust
├── .gitignore
├── docker-compose.yml               # Pebble en Docker
├── docker/
│   ├── pebble-config.json           # Configuración de Pebble
│   └── pebble-root-ca.pem           # CA generado en runtime (no versionar)
├── src/
│   ├── main.rs                      # CLI (clap): issue, renew, show
│   ├── acme/
│   │   ├── mod.rs
│   │   ├── crypto.rs                # ECDSA P-256, JWK, JWS (ES256)
│   │   ├── directory.rs             # Fetch directorio ACME
│   │   ├── client.rs                # AcmeClient: post(), post_as_get()
│   │   ├── account.rs               # Crear/cargar cuenta, persistir JSON
│   │   ├── order.rs                 # Orden, autorización, finalización, polling
│   │   └── challenge.rs             # Servidor HTTP axum para HTTP-01
│   └── cert/
│       ├── mod.rs
│       ├── csr.rs                   # Generar CSR con rcgen
│       └── storage.rs               # Guardar/mostrar PEMs, parse x509
├── test-app/
│   ├── package.json
│   └── server.js                    # Express HTTPS en puerto 8443
└── scripts/
    ├── fetch-pebble-ca.sh
    ├── add-hosts.sh
    └── test-issue.sh
```

### Dependencias Rust (`Cargo.toml`)

```toml
clap = { version = "4", features = ["derive", "env"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rcgen = "0.13"
ring = "0.17"
base64 = "0.22"
sha2 = "0.10"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
axum = "0.7"
x509-parser = "0.16"
hex = "0.4"
```

---

## Comandos ejecutados / Commands Run

### Verificar herramientas

```bash
export PATH="$PATH:/c/Users/jorge/.cargo/bin"
cargo --version        # cargo 1.95.0
rustc --version        # rustc 1.95.0
docker --version       # Docker 29.3.1
node --version         # v24.14.0
```

### Compilar el proyecto

```bash
# Build de desarrollo (rápido, para iterar)
cargo build

# Build de release (optimizado, para uso final)
cargo build --release
```

### Docker — Pebble

```bash
# Levantar Pebble
docker compose up -d

# Ver logs (verificar que arrancó)
docker compose logs pebble

# Parar
docker compose down
```

### Obtener el CA de Pebble (después de cada `docker compose up`)

```bash
curl -sk https://localhost:15000/roots/0 > docker/pebble-root-ca.pem
curl -sk https://localhost:15000/intermediates/0 >> docker/pebble-root-ca.pem
openssl x509 -in docker/pebble-root-ca.pem -noout -subject
```

### Dominios de prueba en /etc/hosts (Windows)

Se añadieron manualmente a `C:\Windows\System32\drivers\etc\hosts`:

```
127.0.0.1  test1.example.com
127.0.0.1  test2.example.com
127.0.0.1  www.test1.example.com
127.0.0.1  www.test2.example.com
```

### Emitir certificados

```bash
# Dominio simple
./target/release/acme-client \
  --acme-url https://localhost:14000/dir \
  --output ./certs \
  --insecure \
  --challenge-bind "0.0.0.0:5002" \
  issue \
  --domain test1.example.com \
  --email admin@example.com

# Multi-dominio (SAN)
./target/release/acme-client \
  --acme-url https://localhost:14000/dir \
  --output ./certs \
  --insecure \
  --challenge-bind "0.0.0.0:5002" \
  issue \
  --domain test2.example.com \
  --domain www.test2.example.com \
  --email admin@example.com
```

### Inspeccionar certificados

```bash
# Via CLI del cliente
./target/release/acme-client --output ./certs show --domain test1.example.com
./target/release/acme-client --output ./certs show --domain test2.example.com

# Via OpenSSL — verificar cadena
openssl verify -CAfile docker/pebble-root-ca.pem certs/test1.example.com/cert.pem
openssl verify -CAfile docker/pebble-root-ca.pem certs/test2.example.com/cert.pem

# Ver SANs
openssl x509 -in certs/test2.example.com/cert.pem -noout -ext subjectAltName
```

### App Express HTTPS

```bash
cd test-app
npm install
DOMAIN=test1.example.com PORT=8443 CERTS_DIR=../certs node server.js
```

### Pruebas con curl

```bash
# Endpoint principal
curl --cacert docker/pebble-root-ca.pem --ssl-no-revoke \
  https://test1.example.com:8443/

# Health check
curl --cacert docker/pebble-root-ca.pem --ssl-no-revoke \
  https://test1.example.com:8443/health
```

---

## Levantar y detener la aplicación / Running & Stopping

### Inicio completo desde cero

```bash
# 1. Exportar PATH de Rust
export PATH="$PATH:/c/Users/jorge/.cargo/bin"

# 2. Ir al directorio del proyecto
cd D:/Master-IA-Dev/06-Bloque6/1-6-30-letsencrypt-client

# 3. Levantar Pebble
docker compose up -d

# 4. Obtener el CA (nuevo en cada arranque de Pebble)
curl -sk https://localhost:15000/roots/0 > docker/pebble-root-ca.pem
curl -sk https://localhost:15000/intermediates/0 >> docker/pebble-root-ca.pem

# 5. Emitir certificado
./target/release/acme-client \
  --acme-url https://localhost:14000/dir \
  --output ./certs \
  --insecure \
  --challenge-bind "0.0.0.0:5002" \
  issue --domain test1.example.com --email admin@example.com

# 6. Levantar Express HTTPS (en otra terminal)
cd test-app && DOMAIN=test1.example.com PORT=8443 CERTS_DIR=../certs node server.js
```

### Parar todo

```bash
# Parar Express: Ctrl+C en su terminal

# Parar Pebble
docker compose down
```

> ⚠️ **Importante:** Cada vez que haces `docker compose down` + `docker compose up`, Pebble genera un **nuevo CA**. Debes repetir el paso 4 (fetch-pebble-ca) y volver a emitir los certificados, porque los anteriores fueron firmados por el CA anterior.
>
> El directorio `certs/.accounts/account.json` también queda obsoleto. Elimínalo para forzar la creación de una nueva cuenta:
> ```bash
> rm -rf certs/.accounts/
> ```

---

## URLs de prueba / Test URLs

Accesibles desde la máquina host (Windows) una vez levantado el stack:

| URL | Descripción |
|-----|-------------|
| `https://test1.example.com:8443/` | Info del certificado en uso |
| `https://test1.example.com:8443/health` | Health check de la app |
| `https://localhost:14000/dir` | Directorio ACME de Pebble |
| `https://localhost:15000/roots/0` | Root CA de Pebble |
| `https://localhost:15000/intermediates/0` | Intermediate CA de Pebble |

---

## Configuración de red / Network Configuration

Este proyecto **no usa VirtualBox ni NAT**. Todo corre localmente en Windows:

- **Pebble** en Docker mapeando puertos `14000` y `15000` al host
- **Challenge server** (Rust/axum) en `0.0.0.0:5002` en el host durante la validación
- **Express HTTPS** en el host en `0.0.0.0:8443`
- **Resolución de dominios** via `C:\Windows\System32\drivers\etc\hosts`

Pebble (en Docker) necesita alcanzar el challenge server en el host. Esto se configura en `docker-compose.yml` con `extra_hosts`, que añade las entradas al `/etc/hosts` del contenedor:

```yaml
extra_hosts:
  - "test1.example.com:host-gateway"
  - "test2.example.com:host-gateway"
  - "www.test1.example.com:host-gateway"
  - "www.test2.example.com:host-gateway"
```

`host-gateway` es un nombre especial de Docker que se resuelve automáticamente a la IP del host desde dentro del contenedor.

---

## Problemas encontrados / Problems & Solutions

| Problema | Solución |
|----------|----------|
| `letsencrypt/pebble` no existe en Docker Hub | Las imágenes están en `ghcr.io/letsencrypt/pebble:latest` |
| `ring::error::Unspecified` no implementa `std::error::Error`, `.context()` de anyhow no compila | Usar `.map_err(\|e\| anyhow::anyhow!("...: {:?}", e))` en lugar de `.context()` |
| `EcdsaKeyPair::public_key()` no encontrado | Faltaba `use ring::signature::KeyPair;` — es un método de trait, no inherente |
| `clap` atributo `env = "..."` no compila | Necesita feature `env` en Cargo.toml: `clap = { features = ["derive", "env"] }` |
| Pebble falla con `invalid command line arguments: pebble -config ...` | El entrypoint de la imagen es `/app`, el `command:` en docker-compose solo debe ser los flags: `-config /test/config/pebble-config.json` |
| Parsing de `Authorization` falla: `missing field 'token'` | Challenges como `tls-alpn-01` y `dns-01` no tienen `token`. Solución: `#[serde(default)]` en el campo `token` del struct `Challenge` |
| `challtestsrv` ocupa el puerto `5002` en el host conflictando con nuestro challenge server | Se eliminó `challtestsrv` del `docker-compose.yml`. No es necesario para HTTP-01 |
| `curl` falla con `CERT_TRUST_REVOCATION_STATUS_UNKNOWN` en Windows | Windows Schannel intenta verificar OCSP y Pebble no publica CRL. Solución: `--ssl-no-revoke` |
| `poll_order_valid` tenía parámetro `client` sin usar y un `break` que rompía la lógica | Refactorizado a `check_order_valid` sin el parámetro innecesario |
| `pem` crate v3 tenía incompatibilidad de API en `storage.rs` | Eliminada la dependencia `pem`. Se usa directamente `x509_parser::pem::parse_x509_pem()` |
| Primer `cargo build` tardó ~3 minutos (descargando 273 crates) | Normal en el primer build. Builds posteriores son segundos |

---

## Resultados y conclusiones / Results & Conclusions

### Lo que fue bien ✅

- El flujo ACME completo funciona de principio a fin contra Pebble
- Dos certificados emitidos con dominios distintos, ambos verificados con `openssl verify`
- El certificado multi-SAN (`test2.example.com` + `www.test2.example.com`) funciona correctamente
- La app Express HTTPS sirve con el certificado emitido; `curl` lo valida con el CA de Pebble
- La cuenta ACME persiste entre ejecuciones (`certs/.accounts/account.json`)
- El servidor HTTP-01 (axum) levanta y para correctamente en cada emisión
- Compilación final sin warnings

### Lo que fue mal / requirió corrección ⚠️

- **Nombre de imagen Docker incorrecto** en el primer borrador (`letsencrypt/pebble` → `ghcr.io/letsencrypt/pebble`)
- **3 errores de compilación en `crypto.rs`** por asumir que ring implementa `std::error::Error` (no lo hace)
- **Entrypoint de Pebble** mal inferido — la documentación de Docker no era obvia
- **challtestsrv incluido innecesariamente** — añadió complejidad sin aportar valor para HTTP-01
- **curl en Windows** requiere `--ssl-no-revoke` — no es obvio y hace fallar silenciosamente sin salida

### Para la próxima sesión 🔜

- Añadir soporte para **DNS-01 challenge** (usando `challtestsrv` como servidor DNS)
- Implementar **renovación automática** (cron o servicio) cuando el cert tiene menos de 30 días de vida
- Probar contra **Let's Encrypt Staging** (no solo Pebble local)
- El campo `Subject` en `show` sale vacío porque rcgen no lo pone en el cert — revisar si es comportamiento esperado de Pebble/rcgen
- Añadir `--renew-if-expires-in <días>` para renovación condicional

---

## Inspección de certificados / Certificate Inspection

### Ver contenido completo con OpenSSL (texto)

```bash
openssl x509 \
  -in certs/test1.example.com/cert.pem \
  -noout -text
```

**Resultado:**

```
Certificate:
    Data:
        Version: 3 (0x2)
        Serial Number:
            1b:66:d5:12:5f:83:e5:d2
        Signature Algorithm: sha256WithRSAEncryption
        Issuer: CN=Pebble Intermediate CA 7a9559
        Validity
            Not Before: May  3 03:23:23 2026 GMT
            Not After : Aug  1 03:23:22 2026 GMT
        Subject:
        Subject Public Key Info:
            Public Key Algorithm: id-ecPublicKey
                Public-Key: (256 bit)
                pub:
                    04:a8:32:eb:d4:1b:5c:ba:9b:d9:7f:21:ce:4c:7b:
                    ad:1c:b2:8e:dd:62:20:53:c8:36:7d:40:fc:42:0d:
                    cb:62:0b:84:79:81:24:c0:4c:57:32:29:fc:21:d8:
                    76:88:51:ec:d8:24:04:f1:2e:23:63:d1:c2:79:6e:
                    e3:47:b4:86:4c
                ASN1 OID: prime256v1
                NIST CURVE: P-256
        X509v3 extensions:
            X509v3 Key Usage: critical
                Digital Signature
            X509v3 Extended Key Usage:
                TLS Web Server Authentication
            X509v3 Basic Constraints: critical
                CA:FALSE
            X509v3 Authority Key Identifier:
                87:2D:E2:2E:20:D1:0B:80:F6:F6:05:B7:12:0D:CE:76:D4:9B:A8:CE
            X509v3 Subject Alternative Name: critical
                DNS:test1.example.com
    Signature Algorithm: sha256WithRSAEncryption
    Signature Value:
        90:6b:f9:89:71:5b:51:98:45:e7:62:e3:23:7b:7f:3a:a1:16:
        ...
```

---

### Ver contenido como JSON con Node.js

```bash
node -e "
const crypto = require('crypto');
const fs = require('fs');

const pem = fs.readFileSync('certs/test1.example.com/cert.pem', 'utf8');
const cert = new crypto.X509Certificate(pem);

const obj = {
  version: 3,
  serialNumber: cert.serialNumber,
  subject: cert.subject || null,
  issuer: cert.issuer,
  validity: {
    notBefore: cert.validFrom,
    notAfter: cert.validTo
  },
  subjectAltName: cert.subjectAltName,
  publicKey: {
    algorithm: cert.publicKey.asymmetricKeyType,
    size: cert.publicKey.asymmetricKeyDetails?.namedCurve || cert.publicKey.asymmetricKeySize
  },
  keyUsage: cert.keyUsage,
  fingerprint: {
    sha1: cert.fingerprint,
    sha256: cert.fingerprint256,
    sha512: cert.fingerprint512
  },
  ca: cert.ca,
  pem: cert.toString()
};

console.log(JSON.stringify(obj, null, 2));
"
```

**Resultado:**

```json
{
  "version": 3,
  "serialNumber": "1B66D5125F83E5D2",
  "subject": null,
  "issuer": "CN=Pebble Intermediate CA 7a9559",
  "validity": {
    "notBefore": "May  3 03:23:23 2026 GMT",
    "notAfter": "Aug  1 03:23:22 2026 GMT"
  },
  "subjectAltName": "DNS:test1.example.com",
  "publicKey": {
    "algorithm": "ec",
    "size": "prime256v1"
  },
  "keyUsage": [
    "1.3.6.1.5.5.7.3.1"
  ],
  "fingerprint": {
    "sha1": "D0:B6:FB:F9:A0:09:28:E5:3A:6A:6A:C5:AA:C6:B6:FE:40:B7:A7:7B",
    "sha256": "7C:6B:AD:EC:A1:E7:6A:9B:02:43:F0:80:76:CD:2B:C4:DA:6A:54:0F:E2:AF:31:47:71:57:2A:CA:12:75:C2:0D",
    "sha512": "05:21:3C:51:9D:97:0B:76:9F:27:63:F0:30:66:4A:93:DF:1D:42:F4:C4:E0:9D:52:EA:B9:65:86:AA:9A:52:3F:EC:1D:2C:19:95:0E:90:99:CA:5C:EA:23:67:26:39:B5:36:7A:57:C2:43:82:35:35:7B:F9:36:74:8D:AA:99:B6"
  },
  "ca": false,
  "pem": "-----BEGIN CERTIFICATE-----\nMIICVjCCAT6g...\n-----END CERTIFICATE-----\n"
}
```

**Notas sobre los campos:**

| Campo | Observación |
|-------|-------------|
| `subject` | `null` — Pebble emite certs sin Subject DN; el dominio va solo en el SAN (comportamiento estándar moderno) |
| `keyUsage[0]` | OID `1.3.6.1.5.5.7.3.1` = `TLS Web Server Authentication` (id-kp-serverAuth) |
| `publicKey.size` | `prime256v1` = curva ECDSA P-256, generada por nuestro cliente Rust con `ring` |
| `ca` | `false` — certificado de entidad final, no de CA |
| `validity` | 90 días — comportamiento por defecto de Pebble |
