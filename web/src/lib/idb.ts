// The app's IndexedDB: the outbox (changes waiting to be sent) and a saved copy of the last
// synced data, so the lists open even when the app starts without a connection (SPEC §6.7).

let db: Promise<IDBDatabase> | null = null

export function openDb(): Promise<IDBDatabase> {
  db ??= new Promise((ok, fail) => {
    const req = indexedDB.open('shoppingcart', 2)
    req.onupgradeneeded = () => {
      const d = req.result
      if (!d.objectStoreNames.contains('outbox')) d.createObjectStore('outbox', { keyPath: 'seq', autoIncrement: true })
      if (!d.objectStoreNames.contains('snapshot')) d.createObjectStore('snapshot')
    }
    req.onsuccess = () => ok(req.result)
    req.onerror = () => fail(req.error)
  })
  return db
}

export async function tx<T>(store: string, mode: IDBTransactionMode, f: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const d = await openDb()
  return new Promise((ok, fail) => {
    const req = f(d.transaction(store, mode).objectStore(store))
    req.onsuccess = () => ok(req.result)
    req.onerror = () => fail(req.error)
  })
}
