import { useEffect, useState } from 'react'
import { useSelector } from 'react-redux'
import { getCertificates, getSystemHealth } from '@/features/admin/api'
import type { RootState } from '@/store'

export default function DashboardPage() {
  const token = useSelector((state: RootState) => state.admin.token)
  const [health, setHealth] = useState('loading')
  const [certificate, setCertificate] = useState('loading')
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
        setHealth(healthPayload.status)
        setCertificate(certPayload.status)
      } catch (loadError) {
        if (canceled) return
        const message = loadError instanceof Error ? loadError.message : 'Failed to load dashboard data.'
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
        <p className="eyebrow">overview</p>
        <h1>Mail Operations Dashboard</h1>
      </header>
      <div className="stats-grid">
        <article className="panel stat-card">
          <h2>Service Health</h2>
          <p className="stat-value">{health}</p>
        </article>
        <article className="panel stat-card">
          <h2>Certificate Status</h2>
          <p className="stat-value">{certificate}</p>
        </article>
        <article className="panel stat-card">
          <h2>Scope</h2>
          <p className="stat-value">Domains + Mailboxes</p>
        </article>
      </div>
      {error ? <p className="error-text">{error}</p> : null}
    </section>
  )
}
