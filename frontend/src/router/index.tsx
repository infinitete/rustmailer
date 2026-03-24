import App from '@/App'
import DashboardPage from '@/features/admin/pages/DashboardPage'
import DomainsPage from '@/features/admin/pages/DomainsPage'
import LoginPage from '@/features/admin/pages/LoginPage'
import MailboxesPage from '@/features/admin/pages/MailboxesPage'
import SystemPage from '@/features/admin/pages/SystemPage'
import { createBrowserRouter } from 'react-router-dom'

export const router = createBrowserRouter([
  {
    element: <App />,
    children: [
      {
        path: '/login',
        element: <LoginPage />,
      },
      {
        path: '/',
        element: <DashboardPage />,
      },
      {
        path: '/domains',
        element: <DomainsPage />,
      },
      {
        path: '/mailboxes',
        element: <MailboxesPage />,
      },
      {
        path: '/system',
        element: <SystemPage />,
      },
    ],
  },
])
