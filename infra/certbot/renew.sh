#!/bin/sh
set -eu

WEBROOT_PATH=/var/www/certbot
CERT_NAME="${CERTBOT_CERT_NAME:-mail.example.com}"
CERT_PATH="/etc/letsencrypt/live/${CERT_NAME}/fullchain.pem"
DOMAIN_ARGS=""

if [ "${CERTBOT_STAGING:-0}" = "1" ]; then
  STAGING_FLAG="--staging"
else
  STAGING_FLAG=""
fi

OLD_IFS=$IFS
IFS=','
for domain in ${CERTBOT_DOMAINS}; do
  DOMAIN_ARGS="$DOMAIN_ARGS -d ${domain}"
done
IFS=$OLD_IFS

if [ ! -f "${CERT_PATH}" ]; then
  certbot certonly \
    --non-interactive \
    --agree-tos \
    --email "${CERTBOT_EMAIL}" \
    --cert-name "${CERT_NAME}" \
    --webroot \
    -w "${WEBROOT_PATH}" \
    ${STAGING_FLAG} \
    ${DOMAIN_ARGS}
fi

while true; do
  certbot renew \
    --non-interactive \
    --webroot \
    -w "${WEBROOT_PATH}" \
    --cert-name "${CERT_NAME}" || true
  sleep 12h
done
