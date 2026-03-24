import { startTransition, useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import { useSelector } from 'react-redux'
import { createDomain, deleteDomain, listDomains, type DomainRecord } from '@/features/admin/api'
import type { RootState } from '@/store'

export default function DomainsPage() {
  const token = useSelector((state: RootState) => state.admin.token)
  const [domainInput, setDomainInput] = useState('')
  const [domains, setDomains] = useState<DomainRecord[]>([])
  const [working, setWorking] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!token) return
    const authToken: string = token

    let canceled = false
    async function load() {
      try {
        const items = await listDomains(authToken)
        if (!canceled) setDomains(items)
      } catch (loadError) {
        if (!canceled) {
          const message = loadError instanceof Error ? loadError.message : 'Failed to load domains.'
          setError(message)
        }
      }
    }

    load()
    return () => {
      canceled = true
    }
  }, [token])

  async function handleCreate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!token || !domainInput.trim()) return

    setWorking(true)
    setError('')
    try {
      const created = await createDomain(token, domainInput.trim())
      startTransition(() => {
        setDomains((current) => [created, ...current.filter((item) => item.id !== created.id)])
        setDomainInput('')
      })
    } catch (createError) {
      const message = createError instanceof Error ? createError.message : 'Failed to create domain.'
      setError(message)
    } finally {
      setWorking(false)
    }
  }

  async function handleDelete(id: number) {
    if (!token) return

    setWorking(true)
    setError('')
    try {
      await deleteDomain(token, id)
      setDomains((current) => current.filter((item) => item.id !== id))
    } catch (deleteError) {
      const message = deleteError instanceof Error ? deleteError.message : 'Failed to delete domain.'
      setError(message)
    } finally {
      setWorking(false)
    }
  }

  return (
    <section className="page-stack">
      <header>
        <p className="eyebrow">directory</p>
        <h1>Managed Domains</h1>
      </header>
      <form className="panel form-grid" onSubmit={handleCreate}>
        <label className="field-label" htmlFor="domain-name">Domain</label>
        <input
          id="domain-name"
          className="field"
          placeholder="example.com"
          value={domainInput}
          onChange={(event) => setDomainInput(event.target.value)}
        />
        <button className="primary-btn" type="submit" disabled={working}>Add Domain</button>
      </form>
      {error ? <p className="error-text">{error}</p> : null}
      <div className="panel">
        <table className="admin-table">
          <thead>
            <tr>
              <th>Name</th>
              <th>Status</th>
              <th className="align-right">Action</th>
            </tr>
          </thead>
          <tbody>
            {domains.length === 0 ? (
              <tr>
                <td colSpan={3} className="muted">No domains available yet.</td>
              </tr>
            ) : (
              domains.map((domain) => (
                <tr key={domain.id}>
                  <td>{domain.name}</td>
                  <td>{domain.enabled ? 'enabled' : 'disabled'}</td>
                  <td className="align-right">
                    <button
                      type="button"
                      className="link-btn"
                      onClick={() => handleDelete(domain.id)}
                      disabled={working}
                    >
                      Remove
                    </button>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </section>
  )
}
