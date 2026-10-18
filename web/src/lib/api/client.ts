// Thin JSON client for /api/v1. The session cookie travels automatically (same origin).

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message)
  }
}

type Listener = () => void
const unauthorizedListeners = new Set<Listener>()
/** Called when the session has ended (401), so the app can show the sign-in screen. */
export const onUnauthorized = (l: Listener) => {
  unauthorizedListeners.add(l)
  return () => unauthorizedListeners.delete(l)
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  let res: Response
  try {
    res = await fetch(`/api/v1${path}`, {
      method,
      credentials: 'same-origin',
      headers: body !== undefined ? { 'Content-Type': 'application/json' } : undefined,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    })
  } catch {
    throw new ApiError(0, 'Could not reach the server')
  }
  if (res.status === 401 && path !== '/auth/login') unauthorizedListeners.forEach((l) => l())
  if (!res.ok) {
    let msg = res.statusText
    try {
      const p = await res.json()
      msg = p.detail ?? p.title ?? msg
    } catch {
      /* not JSON */
    }
    throw new ApiError(res.status, msg)
  }
  if (res.status === 204) return undefined as T
  return res.json() as Promise<T>
}

export const api = {
  get: <T>(p: string) => request<T>('GET', p),
  post: <T>(p: string, b?: unknown) => request<T>('POST', p, b ?? {}),
  patch: <T>(p: string, b: unknown) => request<T>('PATCH', p, b),
  del: <T = void>(p: string) => request<T>('DELETE', p),
}

/** A readable message for any error thrown by `api`. */
export function message(e: unknown): string {
  return e instanceof ApiError ? e.message : 'Something went wrong'
}
