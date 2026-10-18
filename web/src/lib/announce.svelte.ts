// Screen-reader announcements for actions that are otherwise only visual, like
// drag-and-drop results (SPEC §6.5). Rendered by App in an aria-live region.
export const live = $state({ message: '' })

export function announce(message: string) {
  // Clear first so repeating the same message is announced again.
  live.message = ''
  setTimeout(() => (live.message = message), 30)
}
