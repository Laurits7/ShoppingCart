// Live updates (SPEC §6.6): the server's event stream says when something I can see changed,
// and the store then fetches just that with /sync. After a dropped connection the browser
// reconnects by itself; the "hello" it gets then triggers a catch-up too.

let source: EventSource | null = null
let timer: ReturnType<typeof setTimeout> | undefined

/** Listen; `onchange` runs (debounced a little) whenever there may be something new. */
export function connect(onchange: () => void) {
  if (source || typeof EventSource === 'undefined') return
  const nudge = () => {
    clearTimeout(timer)
    timer = setTimeout(onchange, 120)
  }
  source = new EventSource('/api/v1/events')
  for (const name of ['hello', 'change', 'resync']) source.addEventListener(name, nudge)
}

export function disconnect() {
  clearTimeout(timer)
  source?.close()
  source = null
}
