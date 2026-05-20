# Cliente ACME en Rust

Implementar un cliente ACME (RFC 8555) en Rust para obtener certificados TLS de Let's Encrypt (o Pebble en entorno de pruebas).

## Objetivo

Construir una CLI en Rust que automatice el flujo completo de obtención de certificados X.509 via el protocolo ACME, validando dominios mediante HTTP-01 challenge, y almacenando los certificados en disco.

---

## 1. Estructura del Proyecto

```
letsencrypt-client/
├── Cargo.toml
├── Cargo.lock
├── docker-compose.yml          # Pebble + pebble-challtestsrv
├── docker/
│   └── pebble-config.json
├── src/
│   ├── main.rs                 # CLI entrypoint (clap)
│   ├── acme/
│   │   ├── mod.rs
│   │   ├── client.rs           # AcmeClient struct
│   │   ├── account.rs          # Crear/cargar cuenta ACME
│   │   ├── order.rs            # Gestión de órdenes y autorización
│   │   ├── challenge.rs        # Resolver HTTP-01 challenge
│   │   ├── crypto.rs           # Generación de claves, JWK, JWS
│   │   └── directory.rs        # Fetch del directorio ACME
│   └── cert/
│       ├── mod.rs
│       ├── storage.rs          # Guardar/cargar cert + clave privada en disco
│       └── csr.rs              # Generar CSR
├── certs/                      # Certificados generados (gitignored)
│   └── <dominio>/
│       ├── privkey.pem
│       ├── cert.pem
│       └── fullchain.pem
└── test-app/
    ├── package.json
    └── server.js               # App Express con HTTPS
```

---

## 2. Implementación del Cliente ACME

### 2.1 Flujo ACME (RFC 8555)

El cliente debe implementar el flujo completo en orden:

1. **Fetch directory** — GET `<acme-url>/directory` para obtener URLs del servidor
2. **Get nonce** — HEAD `newNonce` para obtener el nonce inicial
3. **Create/load account** — POST `newAccount` con JWK y TOS aceptados; persistir `accountUrl` y clave privada en disco
4. **Create order** — POST `newOrder` con la lista de identificadores (dominios)
5. **Fetch authorizations** — GET de cada URL de autorización devuelta en la orden
6. **Solve HTTP-01 challenge** — levantar un servidor HTTP temporal en puerto 80 que sirva el token en `/.well-known/acme-challenge/<token>`
7. **Notify challenge ready** — POST al challenge URL para indicar que está listo
8. **Poll order status** — hacer polling hasta que la orden pase a `ready` o `valid`
9. **Generate CSR** — crear CSR con los dominios en SAN
10. **Finalize order** — POST `finalize` con el CSR en DER codificado en base64url
11. **Download certificate** — GET de la URL del certificado cuando el estado sea `valid`
12. **Persist** — guardar `privkey.pem`, `cert.pem`, `fullchain.pem` en `certs/<dominio>/`

### 2.2 Criptografía

- Algoritmo de firma: **ECDSA P-256** (ES256)
- Generación de clave de cuenta: `ring` o `rcgen`
- Generación de CSR: `rcgen`
- JWS (JSON Web Signature): construir manualmente o con `jsonwebtoken`
- JWK Thumbprint para token de challenge: SHA-256 sobre JWK canónico

### 2.3 Dependencias Rust sugeridas

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
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
tracing-subscriber = "0.3"
```

### 2.4 CLI (clap)

```
acme-client issue --domain example.com --domain www.example.com \
                  --acme-url https://acme-v02.api.letsencrypt.org/directory \
                  --email admin@example.com \
                  --output ./certs

acme-client renew --domain example.com --output ./certs

acme-client show --domain example.com
```

---

## 3. Entorno de Pruebas con Pebble (Docker)

### 3.1 docker-compose.yml

Levantar dos servicios:
- **pebble** — servidor ACME de pruebas (puerto 14000 HTTPS, 15000 HTTP management)
- **pebble-challtestsrv** — servidor DNS/HTTP para resolver challenges (puerto 8055 management, 5002 HTTP)

```yaml
services:
  pebble:
    image: letsencrypt/pebble:latest
    command: pebble -config /test/config/pebble-config.json
    ports:
      - "14000:14000"
      - "15000:15000"
    environment:
      - PEBBLE_VA_NOSLEEP=1
      - PEBBLE_WFE_NONCEREJECT=0
    volumes:
      - ./docker/pebble-config.json:/test/config/pebble-config.json

  challtestsrv:
    image: letsencrypt/pebble-challtestsrv:latest
    ports:
      - "8055:8055"
      - "5002:5002"
```

### 3.2 pebble-config.json

```json
{
  "pebble": {
    "listenAddress": "0.0.0.0:14000",
    "managementListenAddress": "0.0.0.0:15000",
    "certificate": "test/certs/localhost/cert.pem",
    "privateKey": "test/certs/localhost/key.pem",
    "httpPort": 5002,
    "tlsPort": 5001,
    "ocspResponderURL": "",
    "externalAccountBindingRequired": false
  }
}
```

### 3.3 Variables de entorno para tests

```bash
ACME_URL=https://localhost:14000/dir
PEBBLE_MGMT=https://localhost:15000
CHALLTESTSRV=http://localhost:8055
# Deshabilitar verificación TLS (Pebble usa cert autofirmado)
ACME_INSECURE_TLS=true
```

---

## 4. Dominios de Prueba

Registrar en `/etc/hosts` (o `C:\Windows\System32\drivers\etc\hosts`) los dominios de prueba apuntando a `127.0.0.1`:

```
127.0.0.1  test1.example.com
127.0.0.1  test2.example.com
127.0.0.1  www.test1.example.com
```

Obtener certificados para al menos dos dominios distintos para verificar que el almacenamiento es correcto por dominio.

---

## 5. Aplicación Express de Prueba

Crear `test-app/server.js` — una app Node.js/Express que:

- Levante HTTPS en puerto 8443 usando el certificado generado por el cliente ACME
- Exponga `GET /` devolviendo `{ domain, issued_by, valid_until }`
- Exponga `GET /health`

```bash
# Probar con curl aceptando el CA de Pebble:
curl --cacert docker/pebble-root-ca.pem https://test1.example.com:8443/
```

---

## 6. Orden de Implementación

1. `docker-compose.yml` + config de Pebble — levantar entorno local
2. Módulo `acme/directory.rs` — fetch y parseo del directorio
3. Módulo `acme/crypto.rs` — generación de claves ECDSA, JWK, JWS
4. Módulo `acme/account.rs` — crear cuenta y persistir
5. Módulo `acme/order.rs` — crear orden y gestionar autorización
6. Módulo `acme/challenge.rs` — servidor HTTP temporal para HTTP-01
7. Módulo `cert/csr.rs` + `cert/storage.rs` — CSR y persistencia PEM
8. `main.rs` — CLI con clap integrando todos los módulos
9. Tests de integración contra Pebble
10. `test-app/server.js` — Express HTTPS + pruebas con curl

---

## 7. Criterios de Aceptación

- [ ] `cargo build --release` compila sin errores ni warnings
- [ ] `acme-client issue` obtiene certificado válido contra Pebble
- [ ] Certificados guardados en `certs/<dominio>/{privkey,cert,fullchain}.pem`
- [ ] Al menos 2 dominios distintos gestionados con sus propios certificados
- [ ] App Express responde HTTPS en puerto 8443 con el certificado generado
- [ ] `curl` con el CA de Pebble valida el certificado correctamente
- [ ] El código no hace `unwrap()` sin comentario justificado

---

## 8. Referencias

- RFC 8555 (ACME): https://www.rfc-editor.org/rfc/rfc8555
- Pebble: https://github.com/letsencrypt/pebble
- Pebble Challenge Test Server: https://github.com/letsencrypt/pebble/blob/main/cmd/pebble-challtestsrv/README.md
- rcgen (Rust CSR/cert): https://docs.rs/rcgen
- ring (Rust crypto): https://docs.rs/ring
