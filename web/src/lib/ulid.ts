// Minimal ULID generator (time-ordered, Crockford base32) for client-side ids.
const ENC = '0123456789ABCDEFGHJKMNPQRSTVWXYZ'

export function ulid(now = Date.now()): string {
  let time = ''
  for (let i = 9; i >= 0; i--) {
    time = ENC[now % 32] + time
    now = Math.floor(now / 32)
  }
  const rand = crypto.getRandomValues(new Uint8Array(16))
  let r = ''
  for (let i = 0; i < 16; i++) r += ENC[rand[i] % 32]
  return time + r
}
