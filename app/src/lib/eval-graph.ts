function smoothedPath(
  points: ReadonlyArray<readonly [number, number]>,
): string {
  if (points.length === 0) return ''
  if (points.length === 1) {
    const [x, y] = points[0]
    return `M ${x.toFixed(1)},${y.toFixed(1)} L ${x.toFixed(1)},${y.toFixed(1)}`
  }
  if (points.length === 2) {
    return `M ${points[0][0].toFixed(1)},${points[0][1].toFixed(1)} L ${points[1][0].toFixed(1)},${points[1][1].toFixed(1)}`
  }

  let path = `M ${points[0][0].toFixed(1)},${points[0][1].toFixed(1)}`
  for (let i = 1; i < points.length - 1; i++) {
    const [x0, y0] = points[i]
    const [x1, y1] = points[i + 1]
    const mx = (x0 + x1) / 2
    const my = (y0 + y1) / 2
    path += ` Q ${x0.toFixed(1)},${y0.toFixed(1)} ${mx.toFixed(1)},${my.toFixed(1)}`
  }
  const [lastX, lastY] = points[points.length - 1]
  return `${path} L ${lastX.toFixed(1)},${lastY.toFixed(1)}`
}

/** Connect available evaluations without moving their original ply positions. */
export function graphPaths(
  values: readonly number[],
  width: number,
  height: number,
): { linePath: string; areaPath: string } {
  const points: Array<readonly [number, number]> = []
  for (let i = 0; i < values.length; i++) {
    if (!Number.isFinite(values[i])) continue
    points.push([
      values.length <= 1 ? 0 : (i / (values.length - 1)) * width,
      (1 - values[i] / 100) * height,
    ])
  }
  const linePath = smoothedPath(points)
  if (points.length < 2) return { linePath, areaPath: '' }
  const first = points[0][0].toFixed(1)
  const last = points[points.length - 1][0].toFixed(1)
  const mid = (height / 2).toFixed(1)
  return {
    linePath,
    areaPath: `${linePath} L ${last},${mid} L ${first},${mid} Z`,
  }
}
