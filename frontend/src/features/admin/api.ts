export type DomainRecord = {
  id: number
  name: string
  enabled: boolean
}

export type MailboxRecord = {
  id: number
  domain_id: number
  local_part: string
  email: string
  enabled: boolean
}

export type HealthPayload = {
  status: string
  admin_token_configured: boolean
}

export type CertificatePayload = {
  status: string
  subject_names: string[]
  last_reloaded_at: string | null
}

type RequestConfig = {
  method?: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE'
  token?: string
  body?: unknown
}

const API_BASE_URL = (import.meta.env.VITE_API_BASE_URL as string | undefined) ?? ''

async function requestJson<T>(path: string, config: RequestConfig = {}): Promise<T> {
  const headers: Record<string, string> = {
    Accept: 'application/json',
  }

  if (config.token) {
    headers['x-admin-token'] = config.token
  }

  if (config.body !== undefined) {
    headers['content-type'] = 'application/json'
  }

  const response = await fetch(`${API_BASE_URL}${path}`, {
    method: config.method ?? 'GET',
    headers,
    body: config.body === undefined ? undefined : JSON.stringify(config.body),
  })

  if (!response.ok) {
    const message = await response.text()
    throw new Error(message || `request failed: ${response.status}`)
  }

  if (response.status === 204) {
    return null as T
  }

  return (await response.json()) as T
}

export async function loginAdmin(token: string): Promise<void> {
  await requestJson('/api/admin/login', {
    method: 'POST',
    token,
    body: { token },
  })
}

export async function getSystemHealth(token: string): Promise<HealthPayload> {
  return requestJson<HealthPayload>('/api/admin/system/health', { token })
}

export async function getCertificates(token: string): Promise<CertificatePayload> {
  return requestJson<CertificatePayload>('/api/admin/system/certificates', { token })
}

export async function listDomains(token: string): Promise<DomainRecord[]> {
  return requestJson<DomainRecord[]>('/api/admin/domains', { token })
}

export async function createDomain(token: string, domain: string): Promise<DomainRecord> {
  return requestJson<DomainRecord>('/api/admin/domains', {
    method: 'POST',
    token,
    body: { domain },
  })
}

export async function deleteDomain(token: string, id: number): Promise<void> {
  await requestJson(`/api/admin/domains/${id}`, {
    method: 'DELETE',
    token,
  })
}

export async function listMailboxes(token: string): Promise<MailboxRecord[]> {
  return requestJson<MailboxRecord[]>('/api/admin/mailboxes', { token })
}

export async function createMailbox(
  token: string,
  input: { domain: string; local_part: string; password: string },
): Promise<MailboxRecord> {
  return requestJson<MailboxRecord>('/api/admin/mailboxes', {
    method: 'POST',
    token,
    body: input,
  })
}

export async function deleteMailbox(token: string, id: number): Promise<void> {
  await requestJson(`/api/admin/mailboxes/${id}`, {
    method: 'DELETE',
    token,
  })
}
