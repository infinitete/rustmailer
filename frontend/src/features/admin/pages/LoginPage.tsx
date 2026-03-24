import { startTransition, useState } from 'react'
import type { FormEvent } from 'react'
import { useNavigate } from 'react-router-dom'
import { useDispatch } from 'react-redux'
import { loginAdmin } from '@/features/admin/api'
import { setAdminToken, type AppDispatch } from '@/store'

export default function LoginPage() {
  const [token, setToken] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState('')
  const dispatch = useDispatch<AppDispatch>()
  const navigate = useNavigate()

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()

    if (!token.trim()) {
      setError('Please enter an admin token.')
      return
    }

    setSubmitting(true)
    setError('')

    try {
      await loginAdmin(token.trim())
      dispatch(setAdminToken(token.trim()))
      startTransition(() => {
        navigate('/')
      })
    } catch (submitError) {
      const message = submitError instanceof Error ? submitError.message : 'Login failed.'
      setError(message)
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <main className="login-wrap">
      <section className="panel auth-panel">
        <p className="eyebrow">mail-ops console</p>
        <h1>Admin Access</h1>
        <p className="muted">Use the configured admin token to unlock domain and mailbox operations.</p>
        <form onSubmit={handleSubmit} className="form-stack">
          <label className="field-label" htmlFor="admin-token">Admin token</label>
          <input
            id="admin-token"
            className="field"
            type="password"
            autoComplete="off"
            value={token}
            onChange={(event) => setToken(event.target.value)}
            placeholder="Paste token from server config"
          />
          <button className="primary-btn" type="submit" disabled={submitting}>
            {submitting ? 'Verifying...' : 'Sign in'}
          </button>
          {error ? <p className="error-text">{error}</p> : null}
        </form>
      </section>
    </main>
  )
}
