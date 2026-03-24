import { createSlice, configureStore, type PayloadAction } from '@reduxjs/toolkit'

const ADMIN_TOKEN_STORAGE_KEY = 'rustmailer_admin_token'

export type AdminState = {
  token: string | null
}

const initialToken = globalThis.localStorage?.getItem(ADMIN_TOKEN_STORAGE_KEY) ?? null

const adminSlice = createSlice({
  name: 'admin',
  initialState: {
    token: initialToken,
  } as AdminState,
  reducers: {
    setAdminToken: (state, action: PayloadAction<string>) => {
      state.token = action.payload
      globalThis.localStorage.setItem(ADMIN_TOKEN_STORAGE_KEY, action.payload)
    },
    clearAdminToken: (state) => {
      state.token = null
      globalThis.localStorage.removeItem(ADMIN_TOKEN_STORAGE_KEY)
    },
  },
})

export const store = configureStore({
  reducer: {
    admin: adminSlice.reducer,
  },
})

export const { setAdminToken, clearAdminToken } = adminSlice.actions
export type RootState = ReturnType<typeof store.getState>
export type AppDispatch = typeof store.dispatch
