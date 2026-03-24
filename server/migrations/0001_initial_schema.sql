CREATE TABLE admins (
    id BIGSERIAL PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'admin',
    last_login_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE domains (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    certificate_status TEXT NOT NULL DEFAULT 'pending',
    dns_expectations JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE mailboxes (
    id BIGSERIAL PRIMARY KEY,
    domain_id BIGINT NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    local_part TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    quota_bytes BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (domain_id, local_part)
);

CREATE TABLE mail_folders (
    id BIGSERIAL PRIMARY KEY,
    mailbox_id BIGINT NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    special_use TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (mailbox_id, name)
);

CREATE TABLE messages (
    id BIGSERIAL PRIMARY KEY,
    rfc822 BYTEA NOT NULL,
    subject TEXT,
    from_addr TEXT NOT NULL DEFAULT '',
    size_bytes BIGINT NOT NULL DEFAULT 0,
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE message_delivery (
    id BIGSERIAL PRIMARY KEY,
    message_id BIGINT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    mailbox_id BIGINT NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    folder_id BIGINT NOT NULL REFERENCES mail_folders(id) ON DELETE CASCADE,
    delivered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (message_id, mailbox_id, folder_id)
);

CREATE TABLE message_flags (
    id BIGSERIAL PRIMARY KEY,
    delivery_id BIGINT NOT NULL REFERENCES message_delivery(id) ON DELETE CASCADE,
    flag TEXT NOT NULL,
    UNIQUE (delivery_id, flag)
);

CREATE TABLE imap_folder_state (
    id BIGSERIAL PRIMARY KEY,
    folder_id BIGINT NOT NULL UNIQUE REFERENCES mail_folders(id) ON DELETE CASCADE,
    uid_validity BIGINT NOT NULL,
    uid_next BIGINT NOT NULL
);

CREATE TABLE imap_message_uids (
    id BIGSERIAL PRIMARY KEY,
    delivery_id BIGINT NOT NULL REFERENCES message_delivery(id) ON DELETE CASCADE,
    folder_id BIGINT NOT NULL REFERENCES mail_folders(id) ON DELETE CASCADE,
    uid BIGINT NOT NULL,
    UNIQUE (folder_id, uid),
    UNIQUE (delivery_id, folder_id)
);

CREATE TABLE audit_logs (
    id BIGSERIAL PRIMARY KEY,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    details JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
