import { describe, expect, it } from 'vitest'
import { edgeDelta, insertIndex, neighbours, stepIndex } from './dndmath'

describe('drag and drop maths', () => {
  it('finds the insert index from row midpoints', () => {
    const mids = [10, 30, 50]
    expect(insertIndex(mids, 0)).toBe(0)
    expect(insertIndex(mids, 10)).toBe(0)
    expect(insertIndex(mids, 11)).toBe(1)
    expect(insertIndex(mids, 49)).toBe(2)
    expect(insertIndex(mids, 99)).toBe(3)
    expect(insertIndex([], 5)).toBe(0)
  })

  it('scrolls near the edges only', () => {
    expect(edgeDelta(500, 0, 1000, 60)).toBe(0)
    expect(edgeDelta(30, 0, 1000, 60)).toBe(-10)
    expect(edgeDelta(970, 0, 1000, 60)).toBe(10)
    expect(edgeDelta(0, 0, 1000, 60)).toBeLessThan(edgeDelta(30, 0, 1000, 60))
  })

  it('steps within bounds', () => {
    expect(stepIndex(0, 3, true)).toBe(null)
    expect(stepIndex(0, 3, false)).toBe(1)
    expect(stepIndex(2, 3, false)).toBe(null)
    expect(stepIndex(2, 3, true)).toBe(1)
  })

  it('names the neighbours for the server', () => {
    expect(neighbours(['a', 'b'], 0)).toEqual({ before: 'a' })
    expect(neighbours(['a', 'b'], 1)).toEqual({ after: 'a' })
    expect(neighbours(['a', 'b'], 2)).toEqual({ after: 'b' })
    expect(neighbours([], 0)).toEqual({})
  })
})
