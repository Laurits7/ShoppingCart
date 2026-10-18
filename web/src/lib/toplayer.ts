// While a modal sheet is open, everything outside it is inert: toasts (with their Undo
// buttons) and the screen-reader announcements must live inside the topmost open sheet.
// Sheets call `enter`/`leave`; the hosts move back to the page when none is open.

const HOSTS = ['toasts', 'live-region']
const stack: HTMLElement[] = []

function place() {
  const top = stack[stack.length - 1] ?? document.body
  for (const id of HOSTS) {
    const el = document.getElementById(id)
    if (el && el.parentElement !== top) top.append(el)
  }
}

export function enter(dialog: HTMLElement) {
  if (!stack.includes(dialog)) stack.push(dialog)
  place()
}

export function leave(dialog: HTMLElement) {
  const i = stack.indexOf(dialog)
  if (i >= 0) stack.splice(i, 1)
  place()
}
