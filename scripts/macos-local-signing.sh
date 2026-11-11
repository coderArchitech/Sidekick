#!/bin/sh
# Creates the "Sidekick Local Signing" code-signing certificate in your login
# keychain, for `pnpm build:mac-local`. Run once per Mac. It is self-signed and
# only for builds you run yourself; releases use Developer ID (T-037).
# Remove it any time in Keychain Access (search "Sidekick Local Signing").
set -eu
NAME="Sidekick Local Signing"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

if security find-certificate -c "$NAME" "$KEYCHAIN" >/dev/null 2>&1; then
  echo "\"$NAME\" already exists."
  exit 0
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
cat >"$WORK/cert.cnf" <<CNF
[ req ]
distinguished_name = dn
prompt = no
x509_extensions = ext
[ dn ]
CN = $NAME
O = Sidekick local builds
[ ext ]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
subjectKeyIdentifier = hash
CNF
/usr/bin/openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
  -keyout "$WORK/key.pem" -out "$WORK/cert.pem" -config "$WORK/cert.cnf" 2>/dev/null
PASS=$(/usr/bin/openssl rand -hex 16)
/usr/bin/openssl pkcs12 -export -name "$NAME" -inkey "$WORK/key.pem" -in "$WORK/cert.pem" \
  -out "$WORK/id.p12" -passout "pass:$PASS"
# Only codesign may use the key. macOS asks once; choose Always Allow.
security import "$WORK/id.p12" -k "$KEYCHAIN" -P "$PASS" -T /usr/bin/codesign
echo "Created \"$NAME\". Build with: pnpm build:mac-local"
