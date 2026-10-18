// Which categories and list cards are collapsed, remembered per device.

const KEY = 'sc.collapsed'

function load(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? '[]')
    return Array.isArray(v) ? v : []
  } catch {
    return []
  }
}

class Collapsed {
  ids = $state<string[]>(load())

  has(id: string) {
    return this.ids.includes(id)
  }

  toggle(id: string) {
    this.ids = this.has(id) ? this.ids.filter((c) => c !== id) : [...this.ids, id]
    try {
      localStorage.setItem(KEY, JSON.stringify(this.ids))
    } catch {
      /* private mode: remembered until reload */
    }
  }
}

export const collapsed = new Collapsed()
