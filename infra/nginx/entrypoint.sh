#!/bin/sh
set -eu

CERT_DIR="${TLS_CERT_DIR:-/etc/letsencrypt/live/mail.example.com}"
CERT_PATH="${CERT_DIR}/fullchain.pem"
KEY_PATH="${CERT_DIR}/privkey.pem"
NGINX_CERT_PATH="/etc/nginx/tls/fullchain.pem"
NGINX_KEY_PATH="/etc/nginx/tls/privkey.pem"
SYNC_INTERVAL="${NGINX_CERT_SYNC_INTERVAL:-3600}"

mkdir -p /etc/nginx/tls

resolve_source_certificate() {
  if [ -f "${CERT_PATH}" ] && [ -f "${KEY_PATH}" ]; then
    return 0
  fi

  for candidate in /etc/letsencrypt/live/*; do
    if [ -f "${candidate}/fullchain.pem" ] && [ -f "${candidate}/privkey.pem" ]; then
      CERT_PATH="${candidate}/fullchain.pem"
      KEY_PATH="${candidate}/privkey.pem"
      return 0
    fi
  done

  return 1
}

sync_certificate_files() {
  if resolve_source_certificate; then
    cert_changed=0
    key_changed=0

    if [ ! -f "${NGINX_CERT_PATH}" ] || ! cmp -s "${CERT_PATH}" "${NGINX_CERT_PATH}"; then
      cp "${CERT_PATH}" "${NGINX_CERT_PATH}"
      cert_changed=1
    fi

    if [ ! -f "${NGINX_KEY_PATH}" ] || ! cmp -s "${KEY_PATH}" "${NGINX_KEY_PATH}"; then
      cp "${KEY_PATH}" "${NGINX_KEY_PATH}"
      key_changed=1
    fi

    if [ "${cert_changed}" -eq 1 ] || [ "${key_changed}" -eq 1 ]; then
      nginx -s reload >/dev/null 2>&1 || true
    fi

    return 0
  fi

  if [ ! -f "${NGINX_CERT_PATH}" ] || [ ! -f "${NGINX_KEY_PATH}" ]; then
    openssl req \
      -x509 \
      -nodes \
      -newkey rsa:2048 \
      -days 1 \
      -subj "/CN=localhost" \
      -keyout "${NGINX_KEY_PATH}" \
      -out "${NGINX_CERT_PATH}"
  fi
}

sync_certificate_files

(
  while true; do
    sleep "${SYNC_INTERVAL}"
    sync_certificate_files
  done
) &

exec nginx -g "daemon off;"
