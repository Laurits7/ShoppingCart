// The signed-in user and their preferences (theme), shared by every view.
import { api } from './api/client'
import type { Me } from './api/types/Me'

export type Theme = 'system' | 'light' | 'dark'

class Session {
  me = $state<Me | null>(null)

  get theme(): Theme {
    const t = this.me?.prefs.theme
    return t === 'light' || t === 'dark' ? t : 'system'
  }

  async load() {
    this.me = await api.get<Me>('/me')
    applyTheme(this.theme)
  }

  async setPref(key: string, value: unknown) {
    this.me = await api.patch<Me>('/me/prefs', { [key]: value })
    applyTheme(this.theme)
  }
}

export function applyTheme(t: Theme) {
  if (t === 'system') document.documentElement.removeAttribute('data-theme')
  else document.documentElement.dataset.theme = t
}

export const session = new Session()
