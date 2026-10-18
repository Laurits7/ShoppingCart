<script lang="ts" module>
  // Inline line icons (24×24, stroke), so no icon font or extra requests.
  const PATHS = {
    cart: 'M3 4h3l2.4 11.2a1 1 0 0 0 1 .8h8.8a1 1 0 0 0 1-.7L21 8H7 M10 20.5h.01 M18 20.5h.01',
    gift: 'M4 11h16v9H4z M3 7h18v4H3z M12 7v13 M12 7c-1.5-3-5-3.5-5-1.5S10 7 12 7zm0 0c1.5-3 5-3.5 5-1.5S14 7 12 7z',
    settings:
      'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z',
    plus: 'M12 5v14 M5 12h14',
    x: 'M6 6l12 12 M18 6L6 18',
    logout: 'M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4 M16 17l5-5-5-5 M21 12H9',
    minus: 'M5 12h14',
    camera: 'M4 8h3l2-3h6l2 3h3v11H4z M12 17a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7z',
    barcode: 'M4 5v14 M7 5v14 M10 5v14 M14 5v14 M16 5v14 M20 5v14',
    image: 'M4 5h16v14H4z M4 16l5-5 4 4 2-2 5 5 M15 9h.01',
    search: 'M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14z M20 20l-4-4',
    grip: 'M9 6h.01 M15 6h.01 M9 12h.01 M15 12h.01 M9 18h.01 M15 18h.01',
    move: 'M5 12h14 M13 6l6 6-6 6',
    check: 'M5 12.5l4.5 4.5L19 7.5',
    circle: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z',
    checkCircle: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z M8 12.5l3 3 5-6',
    more: 'M5 12h.01 M12 12h.01 M19 12h.01',
    chevronDown: 'M6 9l6 6 6-6',
    chevronRight: 'M9 6l6 6-6 6',
    star: 'M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z',
    staples: 'M9 6h11 M9 12h11 M9 18h11 M4 6l1 1 2-2 M4 12l1 1 2-2 M4 18l1 1 2-2',
    trash: 'M4 7h16 M10 11v6 M14 11v6 M6 7l1 13h10l1-13 M9 7V4h6v3',
  } as const
  export type IconName = keyof typeof PATHS
</script>

<script lang="ts">
  let { name, size = 22 }: { name: IconName; size?: number } = $props()
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d={PATHS[name]} />
</svg>
