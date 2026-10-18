// A saved copy of the signed-in user and their last synced data (SPEC §6.7): if the app is
// opened without a connection, it starts from this instead of an error. Deleted on sign-out.

import type { Me } from './api/types/Me'
import type { ShoppingData } from './api/types/ShoppingData'
import { tx } from './idb'

export type Snapshot = { me: Me; data: ShoppingData; saved_at: string }

const available = () => typeof indexedDB !== 'undefined'

export async function saveSnapshot(s: Snapshot) {
  if (!available()) return
  await tx('snapshot', 'readwrite', (st) => st.put(s, 'last')).catch(() => {})
}

export async function loadSnapshot(): Promise<Snapshot | null> {
  if (!available()) return null
  return ((await tx('snapshot', 'readonly', (st) => st.get('last')).catch(() => null)) as Snapshot | undefined) ?? null
}

export async function clearSnapshot() {
  if (!available()) return
  await tx('snapshot', 'readwrite', (st) => st.delete('last')).catch(() => {})
}
