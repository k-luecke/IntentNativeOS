#!/usr/bin/env bash
# scripts/generate_test_certs.sh - Generate TLS test certificates for Milestone WEB-01B.3
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CERT_DIR="${ROOT_DIR}/certs"
mkdir -p "${CERT_DIR}"
cd "${CERT_DIR}"

echo "=== Generating Test PKI for Milestone WEB-01B.3 ==="

if [ "${FORCE_REGEN_CERTS:-0}" != "1" ] && [ -f "${CERT_DIR}/server_valid.crt" ] && [ -f "${CERT_DIR}/ca.der" ]; then
    echo "=== Test PKI Certificates already exist. Skipping regeneration. ==="
    exit 0
fi

# Clean old certs
rm -f *.crt *.key *.csr *.srl *.der *.ext index.txt* serial*
mkdir -p newcerts
touch index.txt
echo "1000" > serial

# Minimal OpenSSL CA config
cat << 'EOF' > ca.cnf
[ ca ]
default_ca = CA_default

[ CA_default ]
dir               = .
certs             = $dir
new_certs_dir     = $dir/newcerts
database          = $dir/index.txt
serial            = $dir/serial
certificate       = $dir/ca.crt
private_key       = $dir/ca.key
default_md        = sha256
policy            = policy_loose
email_in_dn       = no
copy_extensions   = copy

[ policy_loose ]
countryName             = optional
stateOrProvinceName     = optional
localityName            = optional
organizationName        = optional
organizationalUnitName  = optional
commonName              = supplied
emailAddress            = optional

[ req ]
distinguished_name = req_distinguished_name
x509_extensions     = v3_ca
prompt              = no

[ req_distinguished_name ]
C  = US
ST = State
L  = City
O  = INOS Test Authority
CN = INOS Root CA

[ v3_ca ]
basicConstraints = critical, CA:true
keyUsage = critical, digitalSignature, cRLSign, keyCertSign

[ v3_server ]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_server

[ alt_server ]
DNS.1 = host.test
IP.1 = 10.0.2.2

[ v3_mismatch ]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_mismatch

[ alt_mismatch ]
DNS.1 = mismatch.test
EOF

# 1. Root CA (Self-Signed)
openssl req -x509 -newkey rsa:2048 -nodes -keyout ca.key -out ca.crt -days 3650 -config ca.cnf -extensions v3_ca

# Export CA in DER format for embedding in net_app
openssl x509 -in ca.crt -outform der -out ca.der

# 2. Rogue / Untrusted Root CA
openssl req -x509 -newkey rsa:2048 -nodes -keyout rogue_ca.key -out rogue_ca.crt -days 3650 \
    -subj "/C=US/ST=State/L=City/O=Rogue Authority/CN=Rogue Root CA" \
    -addext "basicConstraints=critical,CA:TRUE" \
    -addext "keyUsage=critical,keyCertSign,cRLSign"

# 3. Valid Server Certificate (SAN: DNS:host.test, IP:10.0.2.2)
openssl req -newkey rsa:2048 -nodes -keyout server_valid.key -out server_valid.csr \
    -subj "/C=US/ST=State/L=City/O=INOS Host/CN=host.test"

openssl ca -config ca.cnf -batch -extensions v3_server -in server_valid.csr -out server_valid.crt -days 3650

# 4. Expired Server Certificate (Validity 2020 to 2021)
openssl req -newkey rsa:2048 -nodes -keyout server_expired.key -out server_expired.csr \
    -subj "/C=US/ST=State/L=City/O=INOS Host Expired/CN=host.test"

openssl ca -config ca.cnf -batch -extensions v3_server -in server_expired.csr -out server_expired.crt \
    -startdate 200101000000Z -enddate 210101000000Z

# 5. Untrusted Server Certificate (Signed by Rogue CA)
openssl req -newkey rsa:2048 -nodes -keyout server_untrusted.key -out server_untrusted.csr \
    -subj "/C=US/ST=State/L=City/O=Rogue Server/CN=host.test" \
    -addext "subjectAltName = DNS:host.test,IP:10.0.2.2"

openssl x509 -req -in server_untrusted.csr -CA rogue_ca.crt -CAkey rogue_ca.key -CAcreateserial \
    -out server_untrusted.crt -days 3650 -copy_extensions copy

# 6. Hostname Mismatched Server Certificate (SAN: DNS:mismatch.test)
openssl req -newkey rsa:2048 -nodes -keyout server_mismatch.key -out server_mismatch.csr \
    -subj "/C=US/ST=State/L=City/O=INOS Mismatch/CN=mismatch.test"

openssl ca -config ca.cnf -batch -extensions v3_mismatch -in server_mismatch.csr -out server_mismatch.crt -days 3650

# Copy ca.der to userspace/apps/net_app/certs/
NET_APP_CERTS="${ROOT_DIR}/userspace/apps/net_app/certs"
mkdir -p "${NET_APP_CERTS}"
cp ca.der "${NET_APP_CERTS}/ca.der"

echo "=== Generated All Test PKI Certificates Successfully ==="
ls -lh "${CERT_DIR}"/*.crt "${CERT_DIR}"/*.der
