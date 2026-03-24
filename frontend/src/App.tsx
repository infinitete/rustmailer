import { useDispatch, useSelector } from 'react-redux'
import { Navigate, NavLink, Outlet, useLocation } from 'react-router-dom'
import { clearAdminToken, type AppDispatch, type RootState } from '@/store'
import './App.css'

const navItems = [
  { to: '/', label: 'Dashboard' },
  { to: '/domains', label: 'Domains' },
  { to: '/mailboxes', label: 'Mailboxes' },
  { to: '/system', label: 'System' },
]

function App() {
  const token = useSelector((state: RootState) => state.admin.token)
  const location = useLocation()
  const dispatch = useDispatch<AppDispatch>()
  const isLoginRoute = location.pathname === '/login'

  if (!token && !isLoginRoute) {
    return <Navigate to="/login" replace />
  }

  if (token && isLoginRoute) {
    return <Navigate to="/" replace />
  }

  if (isLoginRoute) {
    return <Outlet />
  }

  return (
    <main className="admin-shell">
      <aside className="sidebar">
        <div className="brand-block">
          <p className="eyebrow">rustmailer</p>
          <h2>Mail Ops</h2>
        </div>
        <nav className="sidebar-nav" aria-label="Main">
          {navItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.to === '/'}
              className={({ isActive }) => `nav-link ${isActive ? 'active' : ''}`}
            >
              {item.label}
            </NavLink>
          ))}
        </nav>
        <button
          type="button"
          className="link-btn"
          onClick={() => dispatch(clearAdminToken())}
        >
          Sign out
        </button>
      </aside>
      <section className="content-wrap">
        <div className="content-inner">
          <Outlet />
        </div>
      </section>
    </main>
  )
}

export default App
