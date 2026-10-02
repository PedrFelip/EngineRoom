import { describe, expect, it } from 'vitest'
import { graphPaths } from '../eval-graph'

describe('partial evaluation graph', () => {
  it('keeps out-of-order evaluations at their original positions', () => {
    const values: number[] = []
    values[4] = 75
    values[5] = 50
    const paths = graphPaths(values, 100, 100)
    expect(paths.linePath).toBe('M 80.0,25.0 L 100.0,50.0')
    expect(paths.areaPath).toBe(
      'M 80.0,25.0 L 100.0,50.0 L 100.0,50.0 L 80.0,50.0 Z',
    )
    expect(paths.linePath).not.toContain('NaN')
  })

  it('connects missing positions while preserving their spacing', () => {
    const values = [50, 75, Number.NaN, 25, 50]
    const paths = graphPaths(values, 100, 100)
    expect(paths.linePath).toBe(
      'M 0.0,50.0 Q 25.0,25.0 50.0,50.0 Q 75.0,75.0 87.5,62.5 L 100.0,50.0',
    )
    expect(paths.areaPath.split(' Z')).toHaveLength(2)
  })

  it('handles empty and isolated evaluations', () => {
    expect(graphPaths([], 100, 100)).toEqual({
      linePath: '',
      areaPath: '',
    })
    const values: number[] = []
    values[5] = 60
    expect(graphPaths(values, 100, 100).areaPath).toBe('')
  })
})
