import { useEffect, useState } from 'react'
import { useSelector } from 'react-redux'
import { getCertificates, getSystemHealth, type CertificatePayload, type HealthPayload } from '@/features/admin/api'
import type { RootState } from '@/store'

export default function SystemPage() {
  const token = useSelector((state: RootState) => state.admin.token)
  const [health, setHealth] = useState<HealthPayload | null>(null)
  const [certificates, setCertificates] = useState<CertificatePayload | null>(null)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!token) return
    const authToken: string = token

    let canceled = false
    async function load() {
      try {
        const [healthPayload, certPayload] = await Promise.all([
          getSystemHealth(authToken),
          getCertificates(authToken),
        ])
        if (canceled) return
        setHealth(healthPayload)
        setCertificates(certPayload)
      } catch (loadError) {
        if (canceled) return
        const message = loadError instanceof Error ? loadError.message : 'Failed to load system status.'
        setError(message)
      }
    }

    load()
    return () => {
      canceled = true
    }
  }, [token])

  return (
    <section className="page-stack">
      <header>
        <p className="eyebrow">platform</p>
        <h1>System & Certificate Status</h1>
      </header>
      <div className="stats-grid">
        <article className="panel stat-card">
          <h2>Health</h2>
          <p className="stat-value">{health?.status ?? 'loading'}</p>
          <p className="muted">Admin token configured: {String(health?.admin_token_configured ?? false)}</p>
        </article>
        <article className="panel stat-card">
          <h2>Certificates</h2>
          <p className="stat-value">{certificates?.status ?? 'loading'}</p>
          <p className="muted">
            Subjects: {certificates?.subject_names.length ? certificates.subject_names.join(', ') : 'none'}
          </p>
          <p className="muted">Last reload: {certificates?.last_reloaded_at ?? 'n/a'}</p>
        </article>
      </div>
      {error ? <p className="error-text">{error}</p> : null}
    </section>
  )
}
