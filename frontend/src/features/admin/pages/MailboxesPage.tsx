import { useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import { useSelector } from 'react-redux'
import { createMailbox, deleteMailbox, listMailboxes, type MailboxRecord } from '@/features/admin/api'
import type { RootState } from '@/store'

export default function MailboxesPage() {
  const token = useSelector((state: RootState) => state.admin.token)
  const [domain, setDomain] = useState('')
  const [localPart, setLocalPart] = useState('')
  const [password, setPassword] = useState('')
  const [mailboxes, setMailboxes] = useState<MailboxRecord[]>([])
  const [working, setWorking] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!token) return
    const authToken: string = token

    let canceled = false
    async function load() {
      try {
        const items = await listMailboxes(authToken)
        if (!canceled) setMailboxes(items)
      } catch (loadError) {
        if (!canceled) {
          const message = loadError instanceof Error ? loadError.message : 'Failed to load mailboxes.'
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
    if (!token) return
    if (!domain.trim() || !localPart.trim() || !password.trim()) {
      setError('Domain, local part and password are required.')
      return
    }

    setWorking(true)
    setError('')
    try {
      const mailbox = await createMailbox(token, {
        domain: domain.trim(),
        local_part: localPart.trim(),
        password: password.trim(),
      })
      setMailboxes((current) => [mailbox, ...current.filter((item) => item.id !== mailbox.id)])
      setLocalPart('')
      setPassword('')
    } catch (createError) {
      const message = createError instanceof Error ? createError.message : 'Failed to create mailbox.'
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
      await deleteMailbox(token, id)
      setMailboxes((current) => current.filter((item) => item.id !== id))
    } catch (deleteError) {
      const message = deleteError instanceof Error ? deleteError.message : 'Failed to delete mailbox.'
      setError(message)
    } finally {
      setWorking(false)
    }
  }

  return (
    <section className="page-stack">
      <header>
        <p className="eyebrow">accounts</p>
        <h1>Mailbox Provisioning</h1>
      </header>
      <form className="panel mailbox-grid" onSubmit={handleCreate}>
        <div>
          <label className="field-label" htmlFor="mailbox-domain">Domain</label>
          <input
            id="mailbox-domain"
            className="field"
            placeholder="example.com"
            value={domain}
            onChange={(event) => setDomain(event.target.value)}
          />
        </div>
        <div>
          <label className="field-label" htmlFor="mailbox-local-part">Local part</label>
          <input
            id="mailbox-local-part"
            className="field"
            placeholder="alice"
            value={localPart}
            onChange={(event) => setLocalPart(event.target.value)}
          />
        </div>
        <div>
          <label className="field-label" htmlFor="mailbox-password">Password</label>
          <input
            id="mailbox-password"
            className="field"
            type="password"
            placeholder="password123"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
          />
        </div>
        <button className="primary-btn" type="submit" disabled={working}>Create Mailbox</button>
      </form>
      {error ? <p className="error-text">{error}</p> : null}
      <div className="panel">
        <table className="admin-table">
          <thead>
            <tr>
              <th>Email</th>
              <th>Status</th>
              <th className="align-right">Action</th>
            </tr>
          </thead>
          <tbody>
            {mailboxes.length === 0 ? (
              <tr>
                <td colSpan={3} className="muted">No mailboxes found.</td>
              </tr>
            ) : (
              mailboxes.map((mailbox) => (
                <tr key={mailbox.id}>
                  <td>{mailbox.email}</td>
                  <td>{mailbox.enabled ? 'enabled' : 'disabled'}</td>
                  <td className="align-right">
                    <button
                      className="link-btn"
                      type="button"
                      onClick={() => handleDelete(mailbox.id)}
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
